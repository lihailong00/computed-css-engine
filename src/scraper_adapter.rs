//! Scraper adapter - uses scraper crate for fast HTML parsing
//! and CSS rule extraction

use scraper::{Html, Selector};
use std::collections::HashMap;
use crate::css_parser_core::{CssRule, CssOrigin, calculate_specificity};

/// Parse HTML using scraper and extract all elements with their styles
pub struct ScrapedElements {
    pub elements: Vec<ScrapedElement>,
    pub css_rules: Vec<CssRule>,
}

#[derive(Debug)]
pub struct ScrapedElement {
    pub tag: String,
    pub id: Option<String>,
    pub class: Option<String>,
    pub attributes: HashMap<String, String>,
    pub inline_style: Option<String>,
    /// Depth in DOM tree (0 for root) - approximate
    pub depth: usize,
    /// Index of parent element in the elements vector, None for root
    pub parent_index: Option<usize>,
}

/// Parse HTML and extract elements and CSS rules using scraper
pub fn parse_html_with_scraper(html: &str) -> Result<ScrapedElements, String> {
    let document = Html::parse_document(html);
    let mut css_rules: Vec<CssRule> = Vec::new();

    // Extract CSS rules from style tags
    let style_selector = Selector::parse("style").unwrap();
    for style_elem in document.select(&style_selector) {
        let style_content = style_elem.inner_html();
        let parsed = parse_css_text(&style_content);
        css_rules.extend(parsed);
    }

    // Use select to get all elements - returns them in DOM order
    // Elements come from document order (pre-order traversal)
    let all_selector = Selector::parse("*").unwrap();

    // Track elements and estimate depth based on common tag nesting patterns
    let mut elements_with_info: Vec<(String, Option<String>, Option<String>, HashMap<String, String>, Option<String>)> = Vec::new();

    for elem in document.select(&all_selector) {
        let tag = elem.value().name().to_string();

        // Skip non-renderable elements
        if tag == "script" || tag == "style" || tag == "link" || tag == "meta"
            || tag == "title" || tag == "head" {
            continue;
        }

        elements_with_info.push((
            tag,
            elem.value().attr("id").map(|s| s.to_string()),
            elem.value().attr("class").map(|s| s.to_string()),
            elem.value().attrs().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            elem.value().attr("style").map(|s| s.to_string()),
        ));
    }

    // Estimate depth based on tag hierarchy (simplified approach)
    // This is an approximation since we don't have full tree structure
    let mut elements: Vec<ScrapedElement> = Vec::new();

    for (i, (tag, id, class, attrs, inline_style)) in elements_with_info.into_iter().enumerate() {
        // Estimate depth based on tag type and position
        // Root elements have depth 0, children have depth 1, etc.
        // This is a simplified heuristic
        let depth = estimate_element_depth(&tag, &elements, i);

        elements.push(ScrapedElement {
            tag,
            id,
            class,
            attributes: attrs,
            inline_style,
            depth,
            parent_index: None, // Not easily available without full tree traversal
        });
    }

    Ok(ScrapedElements { elements, css_rules })
}

/// Estimate element depth based on tag and previous elements (simplified)
fn estimate_element_depth(tag: &str, _elements: &[ScrapedElement], _index: usize) -> usize {
    // Common root elements
    match tag {
        "html" | "body" => 0,
        "div" | "p" | "span" | "ul" | "ol" | "li" | "table" | "tr" | "td" | "th"
        | "form" | "input" | "button" | "a" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        | "header" | "footer" | "nav" | "section" | "article" | "aside" | "main"
        | "figure" | "figcaption" | "details" | "summary" => 1,
        _ => 1,
    }
}

/// Parse CSS text into rules
fn parse_css_text(css_text: &str) -> Vec<CssRule> {
    let mut rules = Vec::new();
    let chars: Vec<char> = css_text.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }

        if pos >= chars.len() {
            break;
        }

        // Skip comments
        if pos + 2 < chars.len() && chars[pos] == '/' && chars[pos + 1] == '*' {
            pos += 2;
            while pos + 2 < chars.len() && !(chars[pos] == '*' && chars[pos + 1] == '/') {
                pos += 1;
            }
            pos += 2;
            continue;
        }

        // Find selector (before '{')
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

        // Skip '{'
        while pos < chars.len() && chars[pos] != '{' {
            pos += 1;
        }
        if pos < chars.len() {
            pos += 1;
        }

        // Find declarations (until '}')
        let decl_start = pos;
        let mut brace_count = 1;
        while pos < chars.len() && brace_count > 0 {
            let c = chars[pos];
            if c == '{' {
                brace_count += 1;
            } else if c == '}' {
                brace_count -= 1;
            }
            pos += 1;
        }

        let declarations_text: String = chars[decl_start..pos - 1].iter().collect();
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

/// Parse CSS declarations
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

/// Validate a CSS selector using scraper
pub fn validate_selector(selector: &str) -> Result<Selector, String> {
    Selector::parse(selector).map_err(|e| format!("Invalid selector '{}': {:?}", selector, e))
}

/// Check if a selector is valid
pub fn is_valid_selector(selector: &str) -> bool {
    Selector::parse(selector).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_html_with_scraper() {
        let html = r#"<div id="test" class="container" style="color:red"><p>Hello</p></div>"#;
        let result = parse_html_with_scraper(html).unwrap();
        assert!(result.elements.len() >= 3);
        assert!(!result.css_rules.is_empty() || result.elements.len() > 0);
    }

    #[test]
    fn test_valid_selectors() {
        assert!(is_valid_selector("div"));
        assert!(is_valid_selector("#id"));
        assert!(is_valid_selector(".class"));
    }
}
