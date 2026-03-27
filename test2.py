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