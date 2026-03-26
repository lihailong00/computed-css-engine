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
    /// Depth in DOM tree (0 for root)
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

    // Use scraper's tree to properly traverse DOM with parent tracking
    // The tree field gives us access to the underlying ego_tree
    let tree = document.tree.clone();

    // Collect elements using proper tree traversal
    let mut elements: Vec<ScrapedElement> = Vec::new();
    let mut parent_stack: Vec<usize> = Vec::new(); // Stack of element indices

    traverse_tree(&tree.root(), &mut elements, &mut parent_stack);

    Ok(ScrapedElements { elements, css_rules })
}

/// Recursively traverse the DOM tree and collect elements with parent info
fn traverse_tree(
    node: &ego_tree::NodeRef<scraper::Node>,
    elements: &mut Vec<ScrapedElement>,
    parent_stack: &mut Vec<usize>,
) {
    // Check if this node is an element
    if let Some(elem_data) = node.value().as_element() {
        let tag = elem_data.name();

        // Skip non-renderable elements
        if tag != "script" && tag != "style" && tag != "link" && tag != "meta"
            && tag != "title" && tag != "head" {

            // Extract attributes
            let mut attributes: HashMap<String, String> = HashMap::new();
            let mut id: Option<String> = None;
            let mut class: Option<String> = None;
            let mut inline_style: Option<String> = None;

            for (attr_name, attr_value) in elem_data.attrs() {
                let attr_name = attr_name.to_string();
                let attr_value = attr_value.to_string();
                if attr_name == "id" {
                    id = Some(attr_value.clone());
                } else if attr_name == "class" {
                    class = Some(attr_value.clone());
                } else if attr_name == "style" {
                    inline_style = Some(attr_value.clone());
                }
                attributes.insert(attr_name, attr_value);
            }

            // Get parent index from stack
            let depth = parent_stack.len();
            let parent_index = if parent_stack.is_empty() {
                None
            } else {
                Some(*parent_stack.last().unwrap())
            };

            let elem_index = elements.len();
            elements.push(ScrapedElement {
                tag: tag.to_string(),
                id,
                class,
                attributes,
                inline_style,
                depth,
                parent_index,
            });

            // Push current element to stack (it will be parent for children)
            parent_stack.push(elem_index);
        }
    }

    // Recursively traverse children
    for child in node.children() {
        traverse_tree(&child, elements, parent_stack);
    }

    // Pop from stack when exiting this element (if it was an element we tracked)
    if node.value().as_element().map_or(false, |e| {
        let tag = e.name();
        tag != "script" && tag != "style" && tag != "link" && tag != "meta"
            && tag != "title" && tag != "head"
    }) {
        parent_stack.pop();
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

    #[test]
    fn test_parent_tracking() {
        let html = r#"<html><body><div id="parent"><p id="child">Hello</p></div></body></html>"#;
        let result = parse_html_with_scraper(html).unwrap();

        // Find the p element
        let p_elem = result.elements.iter().find(|e| e.id.as_deref() == Some("child")).unwrap();
        let parent_elem = result.elements.iter().find(|e| e.id.as_deref() == Some("parent")).unwrap();

        // P should have parent_index pointing to div
        assert!(p_elem.parent_index.is_some());
        let parent_idx = p_elem.parent_index.unwrap();
        assert_eq!(result.elements[parent_idx].tag, "div");

        // Depth should be: html=0, body=1, div=2, p=3
        assert_eq!(p_elem.depth, 3);
    }
}
