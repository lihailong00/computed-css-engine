#!/usr/bin/env python3
"""Debug selector matching"""

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

    # 统计有问题的元素
    issues = defaultdict(list)

    for elem in rust_result['elements']:
        tag = elem.get('tag', '').lower()
        attrs = elem.get('attributes', {})
        rust_styles = elem.get('computed_styles', {})

        class_set = frozenset(attrs.get('class', '').split()) if attrs.get('class') else frozenset()
        key = (tag, attrs.get('id'), class_set)

        # 只关注 rust_empty 的情况
        if any(rust_styles.get(p) for p in ['font-size', 'font-weight', 'color', 'display']):
            continue

        # 记录没有样式的元素
        attr_str = f'id={attrs.get("id")} class={attrs.get("class")}' if attrs.get('id') or attrs.get('class') else 'no id/class'
        issues[tag].append(attr_str)

    print('Elements with NO styles by tag:')
    for tag, items in sorted(issues.items(), key=lambda x: -len(x[1]))[:15]:
        print(f'  {tag}: {len(items)}')
        # 看看有没有 id 或 class
        has_id = sum(1 for x in items if 'id=' in x)
        has_class = sum(1 for x in items if 'class=' in x and 'id=' not in x)
        has_none = sum(1 for x in items if 'no id' in x)
        print(f'    with id: {has_id}, with class: {has_class}, plain: {has_none}')

    # 检查 option 标签在 HTML 中是否存在
    print(f'\nTotal option elements in rust result: {len(issues.get("option", []))}')

asyncio.run(test())