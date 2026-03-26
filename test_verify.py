#!/usr/bin/env python3
"""
Test script to verify Rust CSS parser against Playwright computed styles.
禁用JavaScript，统一单位后对比。
"""

import asyncio
import json
import sys
import re
from pathlib import Path
from collections import defaultdict

sys.path.insert(0, '/home/longcoding/dev/project/css_parser/.venv/lib/python3.12/site-packages')

from playwright.async_api import async_playwright


def load_html_file(path: str) -> str:
    with open(path, 'r', encoding='utf-8') as f:
        return f.read()


def rust_parse(html: str, enable_js: bool = False, filter_properties: list = None) -> dict:
    import css_parser
    result = css_parser.parse_html_and_compute_styles(html, enable_js, filter_properties)
    return json.loads(result)


def normalize_css_value(prop: str, value: str) -> str:
    """统一CSS值格式，便于比较"""
    if not value:
        return value

    value = value.strip().lower()

    # 颜色格式统一为 rgb()
    if prop == 'color' or 'color' in prop:
        # #xxxxxx -> rgb()
        hex_match = re.match(r'^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$', value)
        if hex_match:
            r, g, b = hex_match.groups()
            return f'rgb({int(r,16)}, {int(g,16)}, {int(b,16)})'
        # #xxx -> rgb()
        hex_match = re.match(r'^#([0-9a-f])([0-9a-f])([0-9a-f])$', value)
        if hex_match:
            r, g, b = hex_match.groups()
            return f'rgb({int(r*2,16)}, {int(g*2,16)}, {int(b*2,16)})'

    # rgba -> rgb (忽略alpha)
    rgba_match = re.match(r'^rgba?\((\d+),\s*(\d+),\s*(\d+)', value)
    if rgba_match:
        return f'rgb({rgba_match.group(1)}, {rgba_match.group(2)}, {rgba_match.group(3)})'

    # em -> px (基于16px基准) - 只在比较时转换
    if 'em' in value and prop == 'font-size':
        try:
            em_match = re.search(r'([\d.]+)em', value)
            if em_match:
                px = float(em_match.group(1)) * 16
                return f'{px}px'
        except:
            pass

    # 百分比转为固定值 (用于比较)
    if prop == 'background-position':
        if value == 'center center':
            return '50% 50%'
        if value == 'top center' or value == 'center top':
            return '50% 0%'
        if value == 'bottom center' or value == 'center bottom':
            return '50% 100%'
        if value == 'left center' or value == 'center left':
            return '0% 50%'
        if value == 'right center' or value == 'center right':
            return '100% 50%'

    # opacity: 0.8 -> 0.8 (保持1位小数)
    if prop == 'opacity':
        try:
            return f'{float(value):.1f}'
        except:
            pass

    # 去除尾部零
    if value.endswith('px') or value.endswith('%'):
        try:
            num_match = re.search(r'([\d.]+)', value)
            if num_match:
                num = float(num_match.group(1))
                unit = value[len(num_match.group()):]
                if '.' in value:
                    return f'{num:.2f}{unit}'.rstrip('0').rstrip('.')
                return f'{num}{unit}'
        except:
            pass

    return value


async def get_playwright_styles(page, selector: str) -> dict:
    try:
        element = await page.query_selector(selector)
        if element is None:
            return {}
        styles = await element.evaluate("""
            el => {
                const styles = window.getComputedStyle(el);
                const result = {};
                for (let prop of styles) {
                    result[prop] = styles.getPropertyValue(prop);
                }
                return result;
            }
        """)
        return styles
    except:
        return {}


async def test_file(html_path: str, context):
    """测试单个HTML文件"""
    print(f"\n=== Testing: {Path(html_path).name} ===")

    html_content = load_html_file(html_path)
    rust_result = rust_parse(html_content)
    print(f"Rust found {len(rust_result.get('elements', []))} elements")

    page = await context.new_page()
    try:
        await page.set_content(html_content)
        await page.wait_for_load_state('networkidle')

        stats = {
            'total_elements': 0,
            'matched_elements': 0,
            'different_values': 0,
            'rust_missing': 0,
            'pw_missing': 0,
            'property_diffs': defaultdict(int),
            'sample_diffs': []
        }

        for elem in rust_result.get('elements', []):
            tag = elem.get('tag', '')
            attrs = elem.get('attributes', {})
            rust_styles = elem.get('computed_styles', {})

            selector = tag
            if attrs.get('id'):
                selector = f"#{attrs['id']}"
            elif attrs.get('class'):
                cls = attrs['class'].split()[0] if ' ' in attrs['class'] else attrs['class']
                selector = f"{tag}.{cls}"

            stats['total_elements'] += 1
            pw_styles = await get_playwright_styles(page, selector)

            if not pw_styles:
                continue

            stats['matched_elements'] += 1

            all_props = set(rust_styles.keys()) | set(pw_styles.keys())

            for prop in all_props:
                rust_val = rust_styles.get(prop, '')
                pw_val = pw_styles.get(prop, '')

                if not rust_val and pw_val:
                    stats['rust_missing'] += 1
                elif rust_val and not pw_val:
                    stats['pw_missing'] += 1
                else:
                    # 统一格式后比较
                    rust_norm = normalize_css_value(prop, rust_val)
                    pw_norm = normalize_css_value(prop, pw_val)

                    if rust_norm != pw_norm:
                        stats['different_values'] += 1
                        stats['property_diffs'][prop] += 1
                        if len(stats['sample_diffs']) < 20:
                            stats['sample_diffs'].append({
                                'selector': selector,
                                'property': prop,
                                'rust': rust_val,
                                'rust_norm': rust_norm,
                                'playwright': pw_val,
                                'pw_norm': pw_norm
                            })
    finally:
        await page.close()

    return stats


async def main():
    test_pages_dir = Path("/home/longcoding/dev/project/css_parser/test_pages")
    html_files = list(test_pages_dir.glob("*.html"))

    if not html_files:
        print("No HTML test files found!")
        return

    print(f"Found {len(html_files)} HTML test files")
    print("JavaScript: DISABLED\n")

    async with async_playwright() as p:
        # 创建禁用JS的context，设置视口为1920x1080
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context(
            java_script_enabled=False,
            viewport={"width": 1920, "height": 1080}
        )

        total_stats = {
            'total_elements': 0,
            'matched_elements': 0,
            'different_values': 0,
            'rust_missing': 0,
            'pw_missing': 0,
            'property_diffs': defaultdict(int),
            'sample_diffs': []
        }

        for html_file in html_files:
            try:
                stats = await test_file(str(html_file), context)
                total_stats['total_elements'] += stats['total_elements']
                total_stats['matched_elements'] += stats['matched_elements']
                total_stats['different_values'] += stats['different_values']
                total_stats['rust_missing'] += stats['rust_missing']
                total_stats['pw_missing'] += stats['pw_missing']
                total_stats['sample_diffs'].extend(stats['sample_diffs'])
                for prop, count in stats['property_diffs'].items():
                    total_stats['property_diffs'][prop] += count
            except Exception as e:
                print(f"Error testing {html_file}: {e}")
                import traceback
                traceback.print_exc()

        await browser.close()

        print("\n" + "=" * 60)
        print("OVERALL STATISTICS (JS Disabled)")
        print("=" * 60)
        print(f"Total elements found by Rust: {total_stats['total_elements']}")
        print(f"Elements matched with Playwright: {total_stats['matched_elements']}")
        print(f"\nProperty value DIFFERENCES: {total_stats['different_values']}")
        print(f"Properties only in Playwright: {total_stats['rust_missing']}")
        print(f"Properties only in Rust: {total_stats['pw_missing']}")

        if total_stats['property_diffs']:
            print(f"\nTop 15 properties with most differences:")
            sorted_props = sorted(total_stats['property_diffs'].items(), key=lambda x: -x[1])
            for prop, count in sorted_props[:15]:
                print(f"  {prop}: {count}")

        print(f"\nSample differences (first 20):")
        for d in total_stats['sample_diffs'][:20]:
            print(f"  [{d['selector']}] {d['property']}:")
            print(f"    Rust:       '{d['rust']}' -> '{d['rust_norm']}'")
            print(f"    Playwright: '{d['playwright']}' -> '{d['pw_norm']}'")


if __name__ == "__main__":
    asyncio.run(main())
