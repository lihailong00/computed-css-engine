#!/usr/bin/env python3
"""Debug accuracy - analyze differences"""

import asyncio
import json
import re
from collections import defaultdict
from playwright.async_api import async_playwright
import css_parser

def rust_parse(html, filter_properties):
    result = css_parser.parse_html_and_compute_styles(html, False, filter_properties)
    return json.loads(result)

def normalize_css_value(prop, value):
    if not value:
        return value
    value = value.strip().lower()
    if 'color' in prop:
        hex_match = re.match(r'^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$', value)
        if hex_match:
            r, g, b = hex_match.groups()
            return f'rgb({int(r,16)}, {int(g,16)}, {int(b,16)})'
    return value

async def test():
    with open('/home/longcoding/dev/project/css_parser/test_pages/pytorch.html', 'r') as f:
        html = f.read()

    html_no_ext = re.sub(r'<link[^>]*rel=["\']stylesheet["\'][^>]*>', '', html, flags=re.IGNORECASE)
    html_no_ext = re.sub(r'@import\s+["\']([^"\']+)["\'];?', '', html_no_ext, flags=re.IGNORECASE)

    rust_result = rust_parse(html_no_ext, ['font-size', 'font-weight', 'color', 'display'])

    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context(java_script_enabled=False, viewport={'width': 1920, 'height': 1080})
        page = await context.new_page()

        await page.set_content(html_no_ext)
        await page.wait_for_load_state('networkidle')

        pw_all_styles = await page.evaluate('''() => {
            const result = [];
            const elements = document.querySelectorAll('*');
            for (const el of elements) {
                const styles = window.getComputedStyle(el);
                const tag = el.tagName.toLowerCase();
                const id = el.id || null;
                const classList = el.className && typeof el.className === 'string'
                    ? Array.from(el.classList)
                    : [];
                const props = {};
                for (const prop of ['font-size', 'font-weight', 'color', 'display']) {
                    props[prop] = styles.getPropertyValue(prop);
                }
                result.push({ tag, id, classList, props });
            }
            return result;
        }''')

        pw_lookup = {}
        for elem in pw_all_styles:
            key = (elem['tag'], elem['id'], frozenset(elem['classList']))
            pw_lookup[key] = elem['props']

        # 分类统计差异
        stats = {
            'rust_empty_pw_has': 0,  # Rust空，PW有值
            'pw_empty_rust_has': 0,  # PW空，Rust有值
            'both_have': 0,          # 两者都有
            'both_empty': 0,         # 两者都空
            'correct': 0,            # 两者一致
            'incorrect': 0,          # 两者都有但不同
        }

        diff_by_prop = defaultdict(int)
        diff_by_tag = defaultdict(int)
        rust_empty_by_tag = defaultdict(int)

        for elem in rust_result['elements']:
            tag = elem.get('tag', '').lower()
            attrs = elem.get('attributes', {})
            rust_styles = elem.get('computed_styles', {})

            class_set = frozenset(attrs.get('class', '').split()) if attrs.get('class') else frozenset()
            key = (tag, attrs.get('id'), class_set)
            pw_styles = pw_lookup.get(key, {})

            if not pw_styles:
                continue

            for prop in ['font-size', 'font-weight', 'color', 'display']:
                rv = rust_styles.get(prop, '')
                pv = pw_styles.get(prop, '')

                if not rv and pv:
                    stats['rust_empty_pw_has'] += 1
                    rust_empty_by_tag[tag] += 1
                elif rv and not pv:
                    stats['pw_empty_rust_has'] += 1
                elif not rv and not pv:
                    stats['both_empty'] += 1
                else:
                    stats['both_have'] += 1
                    rn = normalize_css_value(prop, rv)
                    pn = normalize_css_value(prop, pv)
                    if rn == pn:
                        stats['correct'] += 1
                    else:
                        stats['incorrect'] += 1
                        diff_by_prop[f'{prop}'] += 1
                        diff_by_tag[f'{tag}/{prop}'] += 1

        print('=== Stats ===')
        for k, v in stats.items():
            print(f'  {k}: {v}')

        print(f'\n=== Diff by property ===')
        for k, v in sorted(diff_by_prop.items(), key=lambda x: -x[1]):
            print(f'  {k}: {v}')

        print(f'\n=== Top diff by tag/property ===')
        for k, v in sorted(diff_by_tag.items(), key=lambda x: -x[1])[:15]:
            print(f'  {k}: {v}')

        print(f'\n=== Rust empty but PW has (top tags) ===')
        for k, v in sorted(rust_empty_by_tag.items(), key=lambda x: -x[1])[:10]:
            print(f'  {k}: {v}')

        # 计算"有效"准确率（排除rust_missing的情况）
        effective_total = stats['correct'] + stats['incorrect']
        effective_correct = stats['correct']
        print(f'\n=== Effective accuracy (excluding rust_missing) ===')
        print(f'  Total: {effective_total}')
        print(f'  Correct: {effective_correct}')
        print(f'  Accuracy: {effective_correct*100/effective_total:.1f}%' if effective_total > 0 else '  N/A')

        await browser.close()

asyncio.run(test())