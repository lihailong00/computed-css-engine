//! CSS Parser module

use crate::html_parser::HtmlElement;
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Cached user-agent stylesheet - parsed once and reused
/// Based on Chromium Blink UA stylesheet (LGPL licensed)
static UA_STYLESHEET: Lazy<Vec<CssRule>> = Lazy::new(|| {
    let css_text = r#"
html { display: block; }
body { display: block; margin: 8px; font-size: 16px; line-height: normal; }
div { display: block; }
p { display: block; margin: 1em 0; }
h1 { display: block; font-size: 2em; font-weight: 700; margin: 0.67em 0; }
h2 { display: block; font-size: 1.5em; font-weight: 700; margin: 0.83em 0; }
h3 { display: block; font-size: 1.17em; font-weight: 700; margin: 1em 0; }
h4 { display: block; font-size: 1em; font-weight: 700; margin: 1.33em 0; }
h5 { display: block; font-size: 0.83em; font-weight: 700; margin: 1.67em 0; }
h6 { display: block; font-size: 0.67em; font-weight: 700; margin: 2.33em 0; }
article, aside, footer, header, hgroup, main, nav, section { display: block; }
blockquote { display: block; margin: 1em 40px; }
q { display: inline; }
q:before { content: open-quote; }
q:after { content: close-quote; }
center { display: block; text-align: center; }
address { display: block; font-style: italic; }
figure { display: block; margin: 1em 40px; }
figcaption { display: block; }
hr { display: block; margin: 0.5em auto; border: 1px inset gray; color: gray; }
ul, menu, dir { display: block; list-style-type: disc; margin: 1em 0; padding-left: 40px; }
ol { display: block; list-style-type: decimal; margin: 1em 0; padding-left: 40px; }
li { display: list-item; }
dl { display: block; margin: 1em 0; }
dt { display: block; }
dd { display: block; margin-left: 40px; }
table { display: table; border-collapse: separate; border-spacing: 2px; box-sizing: border-box; }
thead { display: table-header-group; vertical-align: middle; }
tbody { display: table-row-group; vertical-align: middle; }
tfoot { display: table-footer-group; vertical-align: middle; }
tr { display: table-row; vertical-align: inherit; }
td, th { display: table-cell; vertical-align: inherit; padding: 1px; }
th { font-weight: bold; text-align: center; }
caption { display: table-caption; text-align: center; }
col { display: table-column; }
colgroup { display: table-column-group; }
form { display: block; margin-top: 0; }
fieldset { display: block; margin: 2px; border: 2px groove ButtonFace; padding: 0.35em 0.625em 0.75em; }
legend { display: block; padding: 2px; }
button { display: inline-block; font: -webkit-small-control; color: #000000; background: #ffffff; border: 2px outset ButtonFace; padding: 1px 6px; }
input { display: inline-block; font: -webkit-small-control; color: #000000; background: #ffffff; appearance: auto; }
select { display: inline-block; font: -webkit-small-control; color: #000000; background: #ffffff; appearance: auto; }
textarea { display: inline-block; font: -webkit-small-control; color: #000000; background: #ffffff; appearance: auto; white-space: pre-wrap; font-family: monospace; }
a { color: #0000ee; text-decoration: underline; cursor: pointer; }
a:active { color: #ee0000; }
strong, b { font-weight: bold; }
i, cite, em, var, address, dfn { font-style: italic; }
tt, code, kbd, samp { font-family: monospace; }
pre { display: block; font-family: monospace; white-space: pre; margin: 1em 0; }
mark { background: yellow; color: black; }
small { font-size: smaller; }
big { font-size: larger; }
s, strike, del { text-decoration: line-through; }
sub { vertical-align: sub; font-size: smaller; }
sup { vertical-align: super; font-size: smaller; }
u, ins { text-decoration: underline; }
ruby { display: ruby; }
rt { line-height: normal; }
details { display: block; }
summary { display: block; }
dialog { display: none; }
dialog[open] { display: block; position: absolute; inset: 0; margin: auto; border: solid; padding: 1em; }
[popover]:not(:popover-open) { display: none; }
[popover]:popover-open { display: block; }
slot { display: contents; }
template { display: none; }
head { display: none; }
script { display: none; }
style { display: none; }
link { display: none; }
meta { display: none; }
title { display: none; }
noscript { display: block; }
canvas { display: inline; }
img, embed, object { display: inline; }
video { display: inline-block; object-fit: contain; }
audio { display: inline-block; }
area { display: inline; }
map { display: inline; }
svg { display: inline; }
frame { display: block; }
frameset { display: block; border: 2px groove ButtonFace; }
iframe { display: inline; border: 2px inset ButtonFace; }
fencedframe { display: inline; border: 2px inset ButtonFace; }
param { display: none; }
source { display: none; }
track { display: none; }
wbr { display: inline; }
keygen { display: inline-block; }
progress { display: inline-block; vertical-align: -0.2em; }
meter { display: inline-block; vertical-align: -0.2em; }
option { display: block; }
optgroup { font-weight: bold; display: block; }
span { display: inline; }
i, em, cite, dfn, var, address { font-style: italic; }
b, strong { font-weight: bold; }
s, strike, del { text-decoration: line-through; }
u, ins { text-decoration: underline; }
code, kbd, samp, tt { font-family: monospace; }
small { font-size: smaller; }
big { font-size: larger; }
sub { vertical-align: sub; }
sup { vertical-align: super; }
br { display: inline; }
label { display: inline; cursor: default; }
abbr { display: inline; }
ruby { display: ruby; }
rt { display: ruby-text; font-size: 50%; }
mark { background: yellow; color: black; }
time { display: inline; }
data { display: inline; }
details, summary { display: block; }
menu { display: block; list-style-type: disc; }
main, article, aside, footer, header, nav, section { display: block; }
/* SVG specific */
svg:not(:root) { display: inline; overflow: hidden; }
/* Hidden elements */
area, base, basefont, datalist, head, link, menu, meta, noembed, noframes, noscript, object, param, rp, script, source, style, template, title { display: none; }
/* Text elements */
bdi { display: inline; }
bdo { display: inline; }
rp { display: none; }
rtc { display: inline; }
/* MathML */
math { display: inline; }
mi { display: inline; }
mn { display: inline; }
mo { display: inline; }
ms { display: inline; }
mtext { display: inline; }
annotation-xml { display: inline; }
"#;
    parse_css_text_with_origin(css_text, CssOrigin::UserAgent)
});

/// Get Chromium's default user-agent stylesheet (cached)
pub fn get_user_agent_stylesheet() -> &'static Vec<CssRule> {
    &UA_STYLESHEET
}

/// Parse CSS text with a specific origin
pub fn parse_css_text_with_origin(css_text: &str, origin: CssOrigin) -> Vec<CssRule> {
    let rules = parse_css_text(css_text);
    rules.into_iter().map(|mut r| { r.origin = origin; r }).collect()
}

/// Represents a parsed CSS rule
#[derive(Debug, Clone)]
pub struct CssRule {
    pub selector: String,
    pub declarations: HashMap<String, String>,
    pub specificity: [u32; 3],
    pub origin: CssOrigin,
}

/// CSS rule origin
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssOrigin {
    UserAgent,
    Author,
    User,
}

/// Indexed rules for fast lookup by selector type
#[derive(Debug, Clone, Default)]
pub struct IndexedRules {
    pub id_rules: Vec<usize>,      // indices into all_rules for #id selectors
    pub class_rules: Vec<usize>,   // indices for .class selectors
    pub attr_rules: Vec<usize>,    // indices for [attr] selectors
    pub tag_rules: Vec<usize>,     // indices for tag selectors
    pub universal_rules: Vec<usize>, // indices for * selectors
    pub pseudo_rules: Vec<usize>,  // indices for :pseudo selectors
    pub complex_rules: Vec<usize>, // indices for selectors with combinators
    pub all_rules: Vec<CssRule>,
}

impl IndexedRules {
    pub fn new(rules: Vec<CssRule>) -> Self {
        let mut indexed = IndexedRules {
            all_rules: rules,
            ..Default::default()
        };

        for (idx, rule) in indexed.all_rules.iter().enumerate() {
            let selector = rule.selector.trim();

            // Skip universal selector and comma-separated (handle later)
            if selector.is_empty() {
                continue;
            }

            let first_char = selector.chars().next().unwrap_or(' ');

            // Check for combinators (space, >, +, ~)
            if selector.contains(' ') || selector.starts_with('>') || selector.starts_with('+') || selector.starts_with('~') {
                indexed.complex_rules.push(idx);
            } else if first_char == '#' {
                indexed.id_rules.push(idx);
            } else if first_char == '.' {
                indexed.class_rules.push(idx);
            } else if first_char == '[' {
                indexed.attr_rules.push(idx);
            } else if first_char == ':' {
                indexed.pseudo_rules.push(idx);
            } else if selector == "*" {
                indexed.universal_rules.push(idx);
            } else {
                // Could be tag name or :root, html, etc.
                let lower = selector.to_lowercase();
                if lower == ":root" || lower == "html" || lower == "*" {
                    indexed.tag_rules.push(idx);
                } else {
                    indexed.tag_rules.push(idx);
                }
            }
        }

        indexed
    }

    pub fn len(&self) -> usize {
        self.all_rules.len()
    }
}

/// Extract CSS rules from HTML DOM (from style tags and inline styles)
pub fn extract_css_rules(dom: &HtmlElement) -> Result<Vec<CssRule>, Box<dyn std::error::Error>> {
    let mut rules = Vec::new();
    extract_rules_recursive(dom, &mut rules, CssOrigin::Author)?;
    Ok(rules)
}

fn extract_rules_recursive(
    element: &HtmlElement,
    rules: &mut Vec<CssRule>,
    current_origin: CssOrigin,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check for style tag
    if element.tag_name == "style" {
        // Look for text content in children (stored as _text_style pseudo-element)
        for child in &element.children {
            if child.tag_name == "_text_style" {
                if let Some(ref css_text) = child.text_content {
                    let parsed_rules = parse_css_text(css_text);
                    rules.extend(parsed_rules);
                }
            }
        }
    }

    // Extract inline styles
    if let Some(style) = element.attributes.get("style") {
        if !style.is_empty() {
            let declarations = parse_inline_style(style);
            let selector = format!("[style]");
            rules.push(CssRule {
                selector,
                declarations,
                specificity: [0, 1, 0],
                origin: CssOrigin::Author,
            });
        }
    }

    // Recursively process children
    for child in &element.children {
        extract_rules_recursive(child, rules, current_origin)?;
    }

    Ok(())
}

/// Parse inline style attribute value
pub fn parse_inline_style(style: &str) -> HashMap<String, String> {
    let mut declarations = HashMap::new();

    for part in style.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some(colon_pos) = find_colon_outside_braces(part) {
            let property = part[..colon_pos].trim().to_lowercase();
            let value = part[colon_pos + 1..].trim().to_string();
            if !property.is_empty() {
                declarations.insert(property, value);
            }
        }
    }

    declarations
}

/// Parse CSS text into rules
pub fn parse_css_text(css_text: &str) -> Vec<CssRule> {
    let mut rules = Vec::new();

    // Convert to char indices for proper UTF-8 handling
    let chars: Vec<char> = css_text.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        // Skip whitespace
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }

        if pos >= chars.len() {
            break;
        }

        // Check for comment
        if pos + 2 < chars.len() && chars[pos] == '/' && chars[pos + 1] == '*' {
            pos += 2;
            while pos + 2 < chars.len() && !(chars[pos] == '*' && chars[pos + 1] == '/') {
                pos += 1;
            }
            pos += 2;
            continue;
        }

        // Find the selector (find first '{' not in string)
        let selector_start = pos;
        let mut in_string = false;
        let mut string_char = ' ';

        while pos < chars.len() {
            let c = chars[pos];

            if !in_string && (c == '"' || c == '\'') {
                in_string = true;
                string_char = c;
            } else if in_string && c == string_char {
                in_string = false;
            } else if !in_string && c == '{' {
                break;
            }

            pos += 1;
        }

        let selector: String = chars[selector_start..pos].iter().collect();
        let selector = selector.trim().to_string();

        // Skip whitespace
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }

        // Skip opening brace
        if pos < chars.len() && chars[pos] == '{' {
            pos += 1;
        }

        // Find the declarations (until matching '}')
        let mut brace_count = 1;
        let decl_start = pos;

        while pos < chars.len() && brace_count > 0 {
            let c = chars[pos];
            if c == '{' {
                brace_count += 1;
            } else if c == '}' {
                brace_count -= 1;
            }
            pos += 1;
        }

        // declarations_text excludes the trailing '}'
        let declarations_text: String = chars[decl_start..pos - 1].iter().collect();

        // Parse declarations
        let declarations = parse_declarations(&declarations_text);
        let specificity = calculate_specificity(&selector);

        rules.push(CssRule {
            selector,
            declarations,
            specificity,
            origin: CssOrigin::Author,
        });
    }

    rules
}

/// Parse CSS declarations block
fn parse_declarations(text: &str) -> HashMap<String, String> {
    let mut declarations = HashMap::new();

    for part in text.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some(colon_pos) = find_colon_outside_braces(part) {
            let property = part[..colon_pos].trim().to_lowercase();
            let value = part[colon_pos + 1..].trim().to_string();
            if !property.is_empty() {
                declarations.insert(property, value);
            }
        }
    }

    declarations
}

fn find_colon_outside_braces(s: &str) -> Option<usize> {
    let mut brace_depth: i32 = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => brace_depth += 1,
            ')' => brace_depth = brace_depth.saturating_sub(1),
            ':' if brace_depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Calculate CSS selector specificity
/// Returns [ids, classes/attributes/pseudo-classes, elements/pseudo-elements]
pub fn calculate_specificity(selector: &str) -> [u32; 3] {
    let mut ids = 0u32;
    let mut classes = 0u32;
    let mut elements = 0u32;

    let selector_lower = selector.to_lowercase();
    let selector_chars: Vec<char> = selector_lower.chars().collect();
    let len = selector_chars.len();
    let mut i = 0;

    while i < len {
        let c = selector_chars[i];

        match c {
            '#' => {
                ids += 1;
                i += 1;
                while i < len {
                    let nc = selector_chars[i];
                    if nc.is_alphanumeric() || nc == '-' || nc == '_' {
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
            '.' => {
                classes += 1;
                i += 1;
                while i < len {
                    let nc = selector_chars[i];
                    if nc.is_alphanumeric() || nc == '-' || nc == '_' {
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
            ':' => {
                if i + 1 < len && selector_chars[i + 1] == ':' {
                    // Pseudo element
                    elements += 1;
                    i += 2;
                } else {
                    // Pseudo class
                    classes += 1;
                    i += 1;
                }
                while i < len {
                    let nc = selector_chars[i];
                    if nc.is_alphanumeric() || nc == '-' || nc == '_' || nc == '(' {
                        i += 1;
                        if nc == '(' {
                            // Skip to closing paren
                            while i < len && selector_chars[i] != ')' {
                                i += 1;
                            }
                            if i < len {
                                i += 1;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
            '[' => {
                classes += 1;
                i += 1;
                while i < len && selector_chars[i] != ']' {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
            }
            c if c.is_alphabetic() => {
                elements += 1;
                i += 1;
                while i < len {
                    let nc = selector_chars[i];
                    if nc.is_alphanumeric() || nc == '-' || nc == '_' {
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
            ' ' | '>' | '+' | '~' | ',' => {
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    [ids, classes, elements]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_inline_style() {
        let style = "color: red; font-size: 16px;";
        let declarations = parse_inline_style(style);
        assert_eq!(declarations.get("color"), Some(&"red".to_string()));
        assert_eq!(declarations.get("font-size"), Some(&"16px".to_string()));
    }

    #[test]
    fn test_parse_css_text() {
        let css = "div { color: red; } .class { font-size: 16px; }";
        let rules = parse_css_text(css);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].selector, "div");
        assert_eq!(rules[0].declarations.get("color"), Some(&"red".to_string()));
    }

    #[test]
    fn test_specificity() {
        assert_eq!(calculate_specificity("#id"), [1, 0, 0]);
        assert_eq!(calculate_specificity(".class"), [0, 1, 0]);
        assert_eq!(calculate_specificity("div"), [0, 0, 1]);
        assert_eq!(calculate_specificity("div.class#id"), [1, 1, 1]);
        assert_eq!(calculate_specificity("::before"), [0, 0, 1]);
        assert_eq!(calculate_specificity(":hover"), [0, 1, 0]);
        assert_eq!(calculate_specificity("div > p:first-child"), [0, 1, 2]);
    }

    #[test]
    fn test_parse_css_text_with_utf8() {
        let css = "div { color: 放; }";
        let rules = parse_css_text(css);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].selector, "div");
        assert_eq!(rules[0].declarations.get("color"), Some(&"放".to_string()));
    }
}
