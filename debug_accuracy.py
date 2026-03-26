#!/usr/bin/env python3
"""Debug accuracy test"""

import asyncio
import json
import re
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

        print(f'Rust elements: {len(rust_result["elements"])}')
        print(f'PW elements: {len(pw_all_styles)}')
        print()

        # 看几个具体的rust和pw对比
        count = 0
        for elem in rust_result['elements']:
            tag = elem.get('tag', '').lower()
            attrs = elem.get('attributes', {})
            if not attrs.get('id'):
                continue

            class_set = frozenset(attrs.get('class', '').split()) if attrs.get('class') else frozenset()
            key = (tag, attrs.get('id'), class_set)
            rust_styles = elem.get('computed_styles', {})
            pw_styles = pw_lookup.get(key, {})

            print(f'Element: {tag}#{attrs["id"]}')
            print(f'  Rust:   {rust_styles}')
            print(f'  PW:     {pw_styles}')

            # 逐属性比较
            for prop in ['font-size', 'font-weight', 'color', 'display']:
                rv = rust_styles.get(prop, '')
                pv = pw_styles.get(prop, '')
                if rv and pv:
                    rn = normalize_css_value(prop, rv)
                    pn = normalize_css_value(prop, pv)
                    match = 'OK' if rn == pn else 'DIFF'
                    print(f'    {prop}: {match} (rust={rn}, pw={pn})')

            print()
            count += 1
            if count >= 5:
                break

        await browser.close()

asyncio.run(test())