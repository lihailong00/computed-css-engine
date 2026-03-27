# Computed CSS Engine

High-performance CSS style computation engine for Python. Parses HTML and computes CSS styles for each element using Rust.

## Features

- **Fast**: Built with Rust for high performance
- **CSS Cascade**: Implements full CSS cascade algorithm including specificity, origin priority, and !important
- **CSS Sources**: Supports `<style>` tags and inline `style` attributes
- **Computed Styles**: Returns fully computed CSS property values
- **Python 3.7+**: Supports Python 3.7 through 3.14
- **Cross-platform**: Linux (x86_64, aarch64), Windows, macOS

## Installation

```bash
pip install computed-css-engine
```

Or install from source:

```bash
git clone https://github.com/lihailong00/computed-css-engine.git
cd computed-css-engine
pip install maturin
maturin develop --release
```

## Quick Start

```python
import computed_css_engine

html = """
<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: Arial; }
        .title { color: blue; font-size: 24px; }
        #main { background: #f0f0f0; }
    </style>
</head>
<body>
    <h1 class="title">Hello</h1>
    <div id="main">Content</div>
</body>
</html>
"""

# Get computed styles as JSON
result = computed_css_engine.parse_html_and_compute_styles(html, False, None)
print(result)
```

## API Reference

### `parse_html_and_compute_styles(html, enable_js, filter_properties)`

Parse HTML and compute CSS styles for all elements.

**Parameters:**
- `html` (str): HTML content
- `enable_js` (bool): Reserved for future JavaScript execution support (currently ignored)
- `filter_properties` (List[str] | None): If provided, only compute these CSS properties

**Returns:** JSON string with computed styles

```python
import computed_css_engine
import json

html = "<html><body><p style='color: red;'>Hello</p></body></html>"
result = computed_css_engine.parse_html_and_compute_styles(
    html,
    enable_js=False,
    filter_properties=["color", "font-size"]
)
data = json.loads(result)
print(data)
```

### `parse_html_and_write_styles(html, enable_js, filter_properties, write_to_attr)`

Parse HTML and optionally write computed styles as attributes.

**Parameters:**
- `html` (str): HTML content
- `enable_js` (bool): Reserved for future JavaScript execution support
- `filter_properties` (List[str] | None): CSS properties to compute
- `write_to_attr` (bool): If True, writes styles to `calc-attr` attribute in HTML

**Returns:** Modified HTML string with `calc-attr` attributes (when `write_to_attr=True`)

```python
import computed_css_engine

html = """
<html>
<head><style>.text { color: blue; }</style></head>
<body><p class="text">Hello</p></body>
</html>
"""

# Write styles as HTML attributes
result = computed_css_engine.parse_html_and_write_styles(
    html,
    enable_js=False,
    filter_properties=["color", "font-size"],
    write_to_attr=True
)
print(result)
# Output: <p class="text" calc-attr="color: rgb(0, 0, 255);">Hello</p>
```

## Output Format

Both functions return structured data:

### JSON Format (`parse_html_and_compute_styles`)

```json
{
  "elements": [
    {
      "path": "html > body > p:nth-child(1)",
      "tag": "p",
      "attributes": {"class": "text"},
      "matched_rules": [
        {
          "selector": ".text",
          "specificity": [0, 1, 1],
          "origin": "author",
          "declarations": {"color": "blue"}
        }
      ],
      "computed_styles": {
        "color": "rgb(0, 0, 255)",
        "font-size": "16px",
        "display": "block"
      }
    }
  ]
}
```

### Field Descriptions

| Field | Type | Description |
|-------|------|-------------|
| `path` | str | CSS selector path to uniquely identify the element |
| `tag` | str | HTML tag name |
| `attributes` | dict | Element attributes |
| `matched_rules` | list | All CSS rules that matched this element |
| `matched_rules[].selector` | str | CSS selector text |
| `matched_rules[].specificity` | [int, int, int] | Specificity as [ids, classes, elements] |
| `matched_rules[].origin` | str | "user-agent", "author", or "user" |
| `computed_styles` | dict | Final computed CSS property values |

## Supported CSS Features

### Properties
- Font: `font-size`, `font-weight`, `line-height`
- Color: `color`, `background-color`, `border-color`
- Layout: `display`, `width`, `height`, `max-width`, `min-width`
- Spacing: `margin`, `padding`, `border-width`
- Position: `position`, `top`, `right`, `bottom`, `left`
- Visibility: `visibility`, `z-index`, `opacity`

### Color Formats
- Named colors: `red`, `blue`, `green`, etc.
- Hex: `#fff`, `#ffffff`
- RGB/RGBA: `rgb(255, 0, 0)`, `rgba(0, 0, 0, 0.5)`

### CSS Selectors
- Tag selectors: `div`, `span`, `p`
- Class selectors: `.classname`
- ID selectors: `#elementid`
- Attribute selectors: `[attr=value]`
- Pseudo-classes: `:first-child`, `:nth-child(n)`
- Combinators: ` ` (descendant), `>` (child), `+` (adjacent sibling)

### CSS Cascade
- Specificity calculation
- Origin priority: user-agent < user < author < !important
- !important declaration support
- Inheritance for appropriate properties

## Requirements

- Python >= 3.7
- (No external dependencies - pure Rust implementation)

## License

MIT
