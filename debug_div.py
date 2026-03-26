#!/usr/bin/env python3
"""Debug why div elements have no styles"""

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

        # 获取 playwright 的 div 样式
        divs = await page.evaluate('''() => {
            const divs = [];
            const elements = document.querySelectorAll('div');
            for (const el of elements) {
                const styles = window.getComputedStyle(el);
                divs.push({
                    id: el.id || null,
                    classList: Array.from(el.classList),
                    display: styles.display,
                    color: styles.color,
                    fontSize: styles.fontSize,
                    fontWeight: styles.fontWeight
                });
            }
            return divs;
        }''')

        # 统计
        pw_display_counts = defaultdict(int)
        rust_display_counts = defaultdict(int)

        for pw_div in divs[:20]:  # 只看前20个
            key = (pw_div['id'], tuple(pw_div['classList']))
            print(f"div#{pw_div['id']} class={pw_div['classList'][:2]}")
            print(f"  PW: display={pw_div['display']} color={pw_div['color']} fontSize={pw_div['fontSize']} fontWeight={pw_div['fontWeight']}")

            # 找对应的 rust 元素
            for elem in rust_result['elements']:
                if elem['tag'] == 'div':
                    elem_id = elem['attributes'].get('id')
                    elem_class = elem['attributes'].get('class', '')
                    elem_class_list = tuple(elem_class.split()) if elem_class else ()
                    if elem_id == pw_div['id'] and elem_class_list == tuple(pw_div['classList'][:2]):
                        print(f"  Rust: {elem['computed_styles']}")
                        break
            print()

        await browser.close()

asyncio.run(test())