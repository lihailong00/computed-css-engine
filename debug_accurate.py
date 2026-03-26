#!/usr/bin/env python3
"""Debug accurate stats - element level and property level"""

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

        # Property level stats
        prop_stats = {
            'rust_empty_pw_has': 0,  # 属性级别：Rust空，PW有
            'pw_empty_rust_has': 0,  # 属性级别：PW空，Rust有
            'both_have': 0,          # 属性级别：两者都有
            'correct': 0,            # 属性级别：两者都有且相等
            'incorrect': 0,           # 属性级别：两者都有但不等
        }

        # Element level - elements where ALL 4 props are empty in Rust
        elem_rust_all_empty = 0
        elem_pw_all_empty = 0
        elem_both_have_all = 0
        elem_both_some = 0

        diff_by_tag_prop = defaultdict(int)

        for elem in rust_result['elements']:
            tag = elem.get('tag', '').lower()
            attrs = elem.get('attributes', {})
            rust_styles = elem.get('computed_styles', {})

            class_set = frozenset(attrs.get('class', '').split()) if attrs.get('class') else frozenset()
            key = (tag, attrs.get('id'), class_set)
            pw_styles = pw_lookup.get(key, {})

            if not pw_styles:
                continue

            rust_has = [rust_styles.get(p, '') for p in ['font-size', 'font-weight', 'color', 'display']]
            pw_has = [pw_styles.get(p, '') for p in ['font-size', 'font-weight', 'color', 'display']]

            rust_all_empty = all(not v for v in rust_has)
            pw_all_empty = all(not v for v in pw_has)
            both_have_all = all(r and p for r, p in zip(rust_has, pw_has))

            if rust_all_empty:
                elem_rust_all_empty += 1
            elif pw_all_empty:
                elem_pw_all_empty += 1
            elif both_have_all:
                elem_both_have_all += 1
            else:
                elem_both_some += 1

            # Property level
            for i, prop in enumerate(['font-size', 'font-weight', 'color', 'display']):
                rv = rust_has[i]
                pv = pw_has[i]

                if not rv and pv:
                    prop_stats['rust_empty_pw_has'] += 1
                elif rv and not pv:
                    prop_stats['pw_empty_rust_has'] += 1
                elif rv and pv:
                    prop_stats['both_have'] += 1
                    if normalize_css_value(prop, rv) == normalize_css_value(prop, pv):
                        prop_stats['correct'] += 1
                    else:
                        prop_stats['incorrect'] += 1
                        diff_by_tag_prop[f'{tag}/{prop}'] += 1

        print('=== Element Level ===')
        print(f'  Rust all empty, PW has some: {elem_rust_all_empty}')
        print(f'  PW all empty, Rust has some: {elem_pw_all_empty}')
        print(f'  Both have all 4 props: {elem_both_have_all}')
        print(f'  Both have some props: {elem_both_some}')
        print(f'  Total matched elements: {sum(1 for k, v in pw_lookup.items() if any(v.values()))}')

        print('\n=== Property Level ===')
        print(f'  rust_empty_pw_has: {prop_stats["rust_empty_pw_has"]}')
        print(f'  pw_empty_rust_has: {prop_stats["pw_empty_rust_has"]}')
        print(f'  both_have: {prop_stats["both_have"]}')
        print(f'  correct: {prop_stats["correct"]}')
        print(f'  incorrect: {prop_stats["incorrect"]}')

        if prop_stats['both_have'] > 0:
            print(f'\n  Property Accuracy: {prop_stats["correct"]*100/prop_stats["both_have"]:.1f}%')

        print('\n=== Top differences ===')
        for k, v in sorted(diff_by_tag_prop.items(), key=lambda x: -x[1])[:15]:
            print(f'  {k}: {v}')

        await browser.close()

asyncio.run(test())