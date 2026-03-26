#!/usr/bin/env python3
"""Benchmark and Accuracy Test Module for CSS Parser"""

import asyncio
import json
import re
import time
from pathlib import Path
from collections import defaultdict
import sys
sys.path.insert(0, '/home/longcoding/dev/project/css_parser/.venv/lib/python3.12/site-packages')
from playwright.async_api import async_playwright


def rust_parse(html: str, enable_js: bool = False, filter_properties: list = None) -> dict:
    import css_parser
    result = css_parser.parse_html_and_compute_styles(html, enable_js, filter_properties)
    return json.loads(result)


def load_html_file(path: str) -> str:
    with open(path, "r", encoding="utf-8") as f:
        return f.read()


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


def normalize_css_value(prop: str, value: str) -> str:
    """统一CSS值格式，便于比较"""
    if not value:
        return value

    value = value.strip().lower()

    # 颜色格式统一为 rgb()
    if prop == 'color' or 'color' in prop:
        hex_match = re.match(r'^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$', value)
        if hex_match:
            r, g, b = hex_match.groups()
            return f'rgb({int(r,16)}, {int(g,16)}, {int(b,16)})'
        hex_match = re.match(r'^#([0-9a-f])([0-9a-f])([0-9a-f])$', value)
        if hex_match:
            r, g, b = hex_match.groups()
            return f'rgb({int(r*2,16)}, {int(g*2,16)}, {int(b*2,16)})'

    rgba_match = re.match(r'^rgba?\((\d+),\s*(\d+),\s*(\d+)', value)
    if rgba_match:
        return f'rgb({rgba_match.group(1)}, {rgba_match.group(2)}, {rgba_match.group(3)})'

    # em -> px (基于16px基准)
    if 'em' in value and prop == 'font-size':
        try:
            em_match = re.search(r'([\d.]+)em', value)
            if em_match:
                px = float(em_match.group(1)) * 16
                return f'{px}px'
        except:
            pass

    return value


def new_stats():
    return {
        'total': 0,
        'correct_elements': 0,
        'total_props': 0,
        'correct_props': 0,
        'diff_props': 0,
        'rust_missing': 0
    }


async def test_file_accuracy(html_path: str, context, filter_props: list = None):
    """测试单个HTML文件的准确率

    使用 (tag, id, class_set) 精确匹配元素，确保Rust和Playwright比较的是同一个元素
    """
    html_content = load_html_file(html_path)

    # 移除外部CSS链接，只保留内联style标签（与Rust解析器一致）
    import re
    html_content_no_external = re.sub(
        r'<link[^>]*rel=["\']stylesheet["\'][^>]*>',
        '',
        html_content,
        flags=re.IGNORECASE
    )
    html_content_no_external = re.sub(
        r'@import\s+["\']([^"\']+)["\'];?',
        '',
        html_content_no_external,
        flags=re.IGNORECASE
    )

    rust_result = rust_parse(html_content_no_external, filter_properties=filter_props)

    page = await context.new_page()
    overall = new_stats()
    by_tag = defaultdict(new_stats)

    try:
        await page.set_content(html_content_no_external)
        await page.wait_for_load_state('networkidle')

        # 使用page.evaluate一次获取所有Playwright元素的computed styles
        # 使用 (tag, id, frozenset(classList)) 作为key建立查找表
        pw_all_styles = await page.evaluate("""
            () => {
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
            }
        """)

        # 建立 (tag, id, frozenset(classList)) -> styles 查找表
        pw_lookup = {}
        for elem in pw_all_styles:
            key = (elem['tag'], elem['id'], frozenset(elem['classList']))
            pw_lookup[key] = elem['props']

        # 用同样的方式为Rust结果建立查找表
        rust_lookup = {}
        for elem in rust_result.get('elements', []):
            tag = elem.get('tag', '').lower()
            attrs = elem.get('attributes', {})
            class_set = frozenset(attrs.get('class', '').split()) if attrs.get('class') else frozenset()
            key = (tag, attrs.get('id'), class_set)
            rust_lookup[key] = elem.get('computed_styles', {})

        # 比较：遍历Rust的元素，用精确key匹配Playwright的元素
        for elem in rust_result.get('elements', []):
            tag = elem.get('tag', '').lower() or 'unknown'
            attrs = elem.get('attributes', {})
            rust_styles = elem.get('computed_styles', {})

            class_set = frozenset(attrs.get('class', '').split()) if attrs.get('class') else frozenset()
            key = (tag, attrs.get('id'), class_set)

            pw_styles = pw_lookup.get(key, {})

            # 如果Playwright没有找到这个元素，跳过（说明是浏览器忽略的元素）
            if not pw_styles:
                continue

            overall['total'] += 1
            by_tag[tag]['total'] += 1

            all_props = set(rust_styles.keys()) | set(pw_styles.keys())
            if filter_props:
                all_props = all_props & set(filter_props)

            # Per-element accuracy
            elem_correct = 0
            elem_total = len(all_props)
            elem_diff = 0
            elem_rust_missing = 0

            for prop in all_props:
                rust_val = rust_styles.get(prop, '')
                pw_val = pw_styles.get(prop, '')

                if not rust_val and pw_val:
                    elem_rust_missing += 1
                    elem_diff += 1
                elif rust_val and not pw_val:
                    elem_diff += 1
                else:
                    rust_norm = normalize_css_value(prop, rust_val)
                    pw_norm = normalize_css_value(prop, pw_val)
                    if rust_norm == pw_norm:
                        elem_correct += 1
                    else:
                        elem_diff += 1

            # Element-level accuracy (element matches if ALL properties match)
            if elem_diff == 0 and elem_total > 0:
                overall['correct_elements'] += 1
                by_tag[tag]['correct_elements'] += 1

            overall['total_props'] += elem_total
            overall['correct_props'] += elem_correct
            overall['diff_props'] += elem_diff
            overall['rust_missing'] += elem_rust_missing
            by_tag[tag]['total_props'] += elem_total
            by_tag[tag]['correct_props'] += elem_correct
            by_tag[tag]['diff_props'] += elem_diff
            by_tag[tag]['rust_missing'] += elem_rust_missing

    finally:
        await page.close()

    return {'overall': overall, 'by_tag': dict(by_tag)}


async def benchmark_parsing(html_path: str, iterations: int = 5, filter_props: list = None):
    """性能测试"""
    html_content = load_html_file(html_path)

    times = []
    for _ in range(iterations):
        start = time.perf_counter()
        rust_parse(html_content, filter_properties=filter_props)
        end = time.perf_counter()
        times.append((end - start) * 1000)  # ms

    return {
        'min': min(times),
        'max': max(times),
        'avg': sum(times) / len(times),
        'times': times
    }


async def main():
    test_pages_dir = Path("/home/longcoding/dev/project/css_parser/test_pages")
    html_files = list(test_pages_dir.glob("*.html"))

    if not html_files:
        print("No HTML test files found!")
        return

    print("=" * 70)
    print("CSS Parser - Accuracy & Performance Test")
    print("=" * 70)

    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context(
            java_script_enabled=False,
            viewport={"width": 1920, "height": 1080}
        )

        # 准确率测试
        print("\n[1] Accuracy Test (per-tag breakdown)")
        print("-" * 70)

        all_tag_stats = defaultdict(new_stats)

        for html_file in sorted(html_files):
            try:
                stats = await test_file_accuracy(str(html_file), context)
                for tag, tag_stats in stats['by_tag'].items():
                    all_tag_stats[tag]['total'] += tag_stats['total']
                    all_tag_stats[tag]['correct_elements'] += tag_stats['correct_elements']
                    all_tag_stats[tag]['total_props'] += tag_stats['total_props']
                    all_tag_stats[tag]['correct_props'] += tag_stats['correct_props']
                    all_tag_stats[tag]['diff_props'] += tag_stats['diff_props']
                    all_tag_stats[tag]['rust_missing'] += tag_stats['rust_missing']
            except Exception as e:
                print(f"Error testing {html_file.name}: {e}")
                import traceback
                traceback.print_exc()

        # 打印各标签准确率
        print(f"\n{'Tag':<15} {'Elements':>8} {'ElemAcc%':>10} {'Props':>8} {'PropAcc%':>10}")
        print("-" * 60)

        sorted_tags = sorted(all_tag_stats.items(), key=lambda x: -x[1]['total'])
        for tag, stats in sorted_tags[:20]:  # Top 20
            total = stats['total']
            correct_elem = stats['correct_elements']
            total_props = stats['total_props']
            correct_props = stats['correct_props']

            elem_acc = (correct_elem / total * 100) if total > 0 else 0
            prop_acc = (correct_props / total_props * 100) if total_props > 0 else 0

            print(f"{tag:<15} {total:>8} {elem_acc:>9.1f}% {total_props:>8} {prop_acc:>9.1f}%")

        # 总体准确率
        total_all = sum(s['total'] for s in all_tag_stats.values())
        correct_elem_all = sum(s['correct_elements'] for s in all_tag_stats.values())
        total_props_all = sum(s['total_props'] for s in all_tag_stats.values())
        correct_props_all = sum(s['correct_props'] for s in all_tag_stats.values())

        elem_acc_all = (correct_elem_all / total_all * 100) if total_all > 0 else 0
        prop_acc_all = (correct_props_all / total_props_all * 100) if total_props_all > 0 else 0

        print("-" * 60)
        print(f"{'TOTAL':<15} {total_all:>8} {elem_acc_all:>9.1f}% {total_props_all:>8} {prop_acc_all:>9.1f}%")

        # 性能测试
        print("\n\n[2] Performance Test (parsing time)")
        print("-" * 70)

        for html_file in sorted(html_files):
            try:
                result = await benchmark_parsing(str(html_file), iterations=3)
                print(f"{html_file.name:<40} min: {result['min']:>8.2f}ms  avg: {result['avg']:>8.2f}ms  max: {result['max']:>8.2f}ms")
            except Exception as e:
                print(f"Error benchmarking {html_file.name}: {e}")

        await browser.close()

    print("\n" + "=" * 70)


if __name__ == "__main__":
    asyncio.run(main())
