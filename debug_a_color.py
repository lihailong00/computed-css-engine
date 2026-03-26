#!/usr/bin/env python3
"""Debug a tag color issue"""

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

        # 检查 a 标签的颜色差异
        a_color_diffs = []
        for elem in rust_result['elements']:
            tag = elem.get('tag', '').lower()
            if tag != 'a':
                continue
            attrs = elem.get('attributes', {})
            rust_styles = elem.get('computed_styles', {})

            class_set = frozenset(attrs.get('class', '').split()) if attrs.get('class') else frozenset()
            key = (tag, attrs.get('id'), class_set)
            pw_styles = pw_lookup.get(key, {})

            if not pw_styles:
                continue

            rust_color = rust_styles.get('color', '')
            pw_color = pw_styles.get('color', '')

            if rust_color and pw_color:
                rust_norm = normalize_css_value('color', rust_color)
                pw_norm = normalize_css_value('color', pw_color)
                if rust_norm != pw_norm:
                    a_color_diffs.append({
                        'id': attrs.get('id'),
                        'class': attrs.get('class'),
                        'rust': rust_color,
                        'pw': pw_color,
                        'rust_norm': rust_norm,
                        'pw_norm': pw_norm
                    })

        print(f'Total a/color diffs: {len(a_color_diffs)}')
        print()

        # 统计不同的差异类型
        diff_types = defaultdict(int)
        for d in a_color_diffs:
            diff_types[(d['rust_norm'], d['pw_norm'])] += 1

        print('Top diff types:')
        for (rust, pw), count in sorted(diff_types.items(), key=lambda x: -x[1])[:10]:
            print(f'  Rust={rust} PW={pw}: {count}')

        print()
        print('Sample diffs:')
        for d in a_color_diffs[:5]:
            print(f"  id={d['id']} class={d['class'][:30] if d['class'] else None}")
            print(f"    rust={d['rust']} ({d['rust_norm']})")
            print(f"    pw={d['pw']} ({d['pw_norm']})")

        await browser.close()

asyncio.run(test())