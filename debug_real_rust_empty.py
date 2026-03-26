#!/usr/bin/env python3
"""Debug real rust_empty - count elements where ALL 4 target props are empty"""

import asyncio
import json
import re
from collections import defaultdict
from playwright.async_api import async_playwright
import css_parser

def rust_parse(html, filter_properties):
    result = css_parser.parse_html_and_compute_styles(html, False, filter_properties)
    return json.loads(result)

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

        # 重新统计 rust_empty_pw_has
        # rust_empty = Rust 的 4 个目标属性全为空
        # pw_has = Playwright 的 4 个目标属性至少一个有值

        stats = {
            'rust_all_empty_pw_has_some': 0,  # Rust 4属性全空，PW 至少1个有值
            'rust_has_some_pw_all_empty': 0,  # Rust 至少1个有值，PW 4属性全空
            'both_have': 0,
            'both_empty': 0,
            'correct': 0,
            'incorrect': 0,
        }

        for elem in rust_result['elements']:
            tag = elem.get('tag', '').lower()
            attrs = elem.get('attributes', {})
            rust_styles = elem.get('computed_styles', {})

            class_set = frozenset(attrs.get('class', '').split()) if attrs.get('class') else frozenset()
            key = (tag, attrs.get('id'), class_set)
            pw_styles = pw_lookup.get(key, {})

            if not pw_styles:
                continue

            rust_has_any = any(rust_styles.get(p) for p in ['font-size', 'font-weight', 'color', 'display'])
            pw_has_any = any(pw_styles.get(p) for p in ['font-size', 'font-weight', 'color', 'display'])

            if not rust_has_any and pw_has_any:
                stats['rust_all_empty_pw_has_some'] += 1
            elif rust_has_any and not pw_has_any:
                stats['rust_has_some_pw_all_empty'] += 1
            elif rust_has_any and pw_has_any:
                stats['both_have'] += 1
                # 检查是否 correct
                all_correct = True
                for prop in ['font-size', 'font-weight', 'color', 'display']:
                    rv = rust_styles.get(prop, '')
                    pv = pw_styles.get(prop, '')
                    if rv and pv:
                        # 简单的规范化比较
                        def norm(v):
                            v = v.strip().lower()
                            # 简化比较
                            return v
                        if norm(rv) != norm(pv):
                            all_correct = False
                            break
                if all_correct:
                    stats['correct'] += 1
                else:
                    stats['incorrect'] += 1
            else:
                stats['both_empty'] += 1

        print('=== Revised Stats ===')
        for k, v in stats.items():
            print(f'  {k}: {v}')

        # 计算 effective accuracy
        effective_total = stats['correct'] + stats['incorrect']
        effective_correct = stats['correct']
        print(f'\n=== Effective Accuracy ===')
        print(f'  Total: {effective_total}')
        print(f'  Correct: {effective_correct}')
        print(f'  Accuracy: {effective_correct*100/effective_total:.1f}%' if effective_total > 0 else '  N/A')

        await browser.close()

asyncio.run(test())