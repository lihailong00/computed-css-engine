#!/usr/bin/env python3
"""Final accuracy test - using exact matching logic"""

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
        rgba_match = re.match(r'^rgba?\((\d+),\s*(\d+),\s*(\d+)', value)
        if rgba_match:
            return f'rgb({rgba_match.group(1)}, {rgba_match.group(2)}, {rgba_match.group(3)})'
    if 'em' in value and prop == 'font-size':
        try:
            em_match = re.search(r'([\d.]+)em', value)
            if em_match:
                return f'{float(em_match.group(1)) * 16}px'
        except:
            pass
    return value

async def test_file(html_path, filter_props):
    with open(html_path, 'r', encoding='utf-8') as f:
        html = f.read()

    html = re.sub(r'<link[^>]*rel=["\']stylesheet["\'][^>]*>', '', html, flags=re.IGNORECASE)
    html = re.sub(r'@import\s+["\']([^"\']+)["\'];?', '', html, flags=re.IGNORECASE)

    rust_result = rust_parse(html, filter_props)

    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context(java_script_enabled=False, viewport={'width': 1920, 'height': 1080})
        page = await context.new_page()
        await page.set_content(html)
        await page.wait_for_load_state('networkidle')

        pw_all = await page.evaluate('''() => {
            const result = [];
            const elements = document.querySelectorAll('*');
            for (const el of elements) {
                const styles = window.getComputedStyle(el);
                const tag = el.tagName.toLowerCase();
                const id = el.id || null;
                const classList = el.className && typeof el.className === 'string'
                    ? Array.from(el.classList) : [];
                const props = {};
                for (const prop of ['font-size', 'font-weight', 'color', 'display']) {
                    props[prop] = styles.getPropertyValue(prop);
                }
                result.push({ tag, id, classList, props });
            }
            return result;
        }''')

        pw_lookup = {(e['tag'], e['id'], frozenset(e['classList'])): e['props'] for e in pw_all}

        total_props = 0
        correct_props = 0
        diff_props = 0
        rust_missing = 0

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
                if rv and pv:
                    total_props += 1
                    if normalize_css_value(prop, rv) == normalize_css_value(prop, pv):
                        correct_props += 1
                    else:
                        diff_props += 1
                elif not rv and pv:
                    rust_missing += 1

        await browser.close()
        return {
            'total': total_props,
            'correct': correct_props,
            'diff': diff_props,
            'rust_missing': rust_missing
        }

async def main():
    test_pages_dir = "/home/longcoding/dev/project/css_parser/test_pages"
    html_files = [
        "pytorch.html",
        "simple_page.html",
        "bootstrap_example.html",
        "html5_test.html",
        "inline_style_test.html",
        "w3c_wcag.html"
    ]

    print("=" * 70)
    print("CSS Parser - Final Accuracy & Performance Test")
    print("=" * 70)

    all_stats = {'total': 0, 'correct': 0, 'diff': 0, 'rust_missing': 0}

    for html_file in html_files:
        path = f"{test_pages_dir}/{html_file}"
        try:
            stats = await test_file(path, ['font-size', 'font-weight', 'color', 'display'])
            all_stats['total'] += stats['total']
            all_stats['correct'] += stats['correct']
            all_stats['diff'] += stats['diff']
            all_stats['rust_missing'] += stats['rust_missing']

            acc = stats['correct'] / stats['total'] * 100 if stats['total'] > 0 else 0
            print(f"{html_file:<35} Props: {stats['total']:>5}  Correct: {stats['correct']:>5}  Acc: {acc:.1f}%")
        except Exception as e:
            print(f"{html_file:<35} Error: {e}")

    print("-" * 70)
    total = all_stats['total']
    correct = all_stats['correct']
    diff = all_stats['diff']
    rust_missing = all_stats['rust_missing']

    effective_total = correct + diff
    effective_acc = correct / effective_total * 100 if effective_total > 0 else 0

    print(f"{'TOTAL':<35} Props: {total:>5}  Correct: {correct:>5}  Diff: {diff:>5}  Missing: {rust_missing:>5}")
    print(f"\nEffective Accuracy (excluding rust_missing): {effective_acc:.1f}%")
    print("=" * 70)

asyncio.run(main())