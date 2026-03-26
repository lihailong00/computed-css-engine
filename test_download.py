#!/usr/bin/env python3
"""Download pytorch.org HTML with JS enabled to capture dynamically loaded content."""

import asyncio
import sys
sys.path.insert(0, '/home/longcoding/dev/project/css_parser/.venv/lib/python3.12/site-packages')

from playwright.async_api import async_playwright

async def download_page():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        page = await browser.new_page()

        print("Downloading pytorch.org with JS enabled...")
        await page.goto("https://pytorch.org/", wait_until="networkidle")
        await page.wait_for_timeout(2000)  # Wait for dynamic content

        content = await page.content()

        with open("/home/longcoding/dev/project/css_parser/test_pages/pytorch.html", "w", encoding="utf-8") as f:
            f.write(content)

        print(f"Downloaded {len(content)} bytes")
        await browser.close()

if __name__ == "__main__":
    asyncio.run(download_page())
