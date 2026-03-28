//! CSS Parser Library
//! Parses HTML and computes CSS styles for each element.

pub mod html_parser;
pub mod css_parser_core;
pub mod cascade;
pub mod computed;
pub mod pseudo;
pub mod style_tree;
pub mod js_executor;
pub mod scraper_adapter;

use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents an element's computed styles and matched rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementStyles {
    /// CSS selector path to uniquely identify this element
    pub path: String,
    /// HTML tag name
    pub tag: String,
    /// Element attributes
    pub attributes: HashMap<String, String>,
    /// All CSS rules that matched this element
    pub matched_rules: Vec<MatchedRule>,
    /// Final computed styles
    pub computed_styles: HashMap<String, String>,
}

/// A matched CSS rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedRule {
    /// The CSS selector text
    pub selector: String,
    /// Specificity as [ids, classes, elements]
    pub specificity: [u32; 3],
    /// Origin: "user-agent", "author", or "user"
    pub origin: String,
    /// Declarations from this rule
    pub declarations: HashMap<String, String>,
}

/// Main result structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub elements: Vec<ElementStyles>,
}

/// Parse HTML and compute CSS styles, optionally writing styles to calc-attr attribute.
/// Returns modified HTML string with styles written to calc-attr attribute.
/// - enable_js: (reserved) attempt to execute JavaScript and capture CSS variable modifications
/// - filter_properties: if Some, only return these properties (e.g., ["font-size", "color"])
/// - write_to_attr: if true, writes computed styles to each element's calc-attr attribute
#[pyo3::pyfunction]
pub fn parse_html_and_write_styles(
    html: &str,
    enable_js: bool,
    filter_properties: Option<Vec<String>>,
    write_to_attr: bool,
) -> String {
    let filter_clone = filter_properties.clone();
    match compute_styles(html, enable_js, filter_properties) {
        Ok(result) => {
            if write_to_attr {
                write_styles_to_html_attr(html, &result.elements, filter_clone.as_deref())
            } else {
                serde_json::to_string(&result).unwrap_or_else(|e| e.to_string())
            }
        }
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Write computed styles to HTML elements' calc-attr attribute
/// Optimized: single-pass serialization without string replacement
fn write_styles_to_html_attr(
    html: &str,
    elements: &[ElementStyles],
    filter_properties: Option<&[String]>,
) -> String {
    use scraper::Html;

    // Parse HTML once
    let document = Html::parse_document(html);
    let tree = document.tree.clone();

    // Target properties
    let target_props: Vec<String> = filter_properties.map(|props| {
        props.iter().map(|s| s.to_string()).collect()
    }).unwrap_or_else(|| {
        vec!["font-size".to_string(), "font-weight".to_string(), "color".to_string(), "display".to_string()]
    });

    // Build a map from element key to calc-attr string
    let mut calc_attr_map: HashMap<String, String> = HashMap::new();
    for elem in elements {
        if elem.computed_styles.is_empty() {
            continue;
        }

        let mut attrs: Vec<String> = Vec::new();
        for prop in &target_props {
            if let Some(value) = elem.computed_styles.get(prop) {
                if !value.is_empty() && !value.contains("display: none") {
                    attrs.push(format!("{}: {}", prop, value));
                }
            }
        }

        if !attrs.is_empty() {
            let calc_attr = format!("calc-attr=\"{}\"", attrs.join("; "));
            let key = element_key(&elem.tag, &elem.attributes);
            calc_attr_map.insert(key, calc_attr);
        }
    }

    // Serialize tree with calc-attr inserted in single pass
    serialize_with_attrs(tree.root(), &calc_attr_map)
}

/// Create a unique key for an element based on tag and important attributes
/// Key format: "tag#id.class" or "tag.class" or "tag" for simple matching
fn element_key(tag: &str, attrs: &HashMap<String, String>) -> String {
    let mut key = tag.to_string();
    if let Some(id) = attrs.get("id") {
        key.push('#');
        key.push_str(id);
    }
    if let Some(class) = attrs.get("class") {
        // Use only first class (same as original build_selector_for_element)
        let first_class = class.split_whitespace().next().unwrap_or("");
        if !first_class.is_empty() {
            key.push('.');
            key.push_str(first_class);
        }
    }
    key
}

/// Serialize tree to HTML string, inserting calc-attr for matching elements
fn serialize_with_attrs(
    node: ego_tree::NodeRef<scraper::Node>,
    calc_attr_map: &HashMap<String, String>,
) -> String {
    let mut output = String::new();
    serialize_node(node, calc_attr_map, &mut output);
    output
}

fn serialize_node(
    node: ego_tree::NodeRef<scraper::Node>,
    calc_attr_map: &HashMap<String, String>,
    output: &mut String,
) {
    match node.value() {
        scraper::Node::Element(elem) => {
            let tag = elem.name();

            // Skip non-renderable elements for style calculation
            let is_renderable = !matches!(
                tag,
                "script" | "style" | "link" | "meta" | "title" | "head"
            );

            output.push('<');
            output.push_str(tag);

            // Collect attributes and build key
            let mut key = tag.to_string();

            for (name, value) in elem.attrs() {
                let name = name.to_string();
                let value = value.to_string();

                // Track id and class for key (only first class)
                if name == "id" {
                    key.push('#');
                    key.push_str(&value);
                } else if name == "class" {
                    let first_class = value.split_whitespace().next().unwrap_or("");
                    if !first_class.is_empty() {
                        key.push('.');
                        key.push_str(first_class);
                    }
                }

                // Output attribute (skip style attribute - we'll write calc-attr instead)
                if name != "style" {
                    output.push(' ');
                    output.push_str(&name);
                    output.push_str("=\"");
                    output.push_str(&value.replace('"', "&quot;"));
                    output.push('"');
                }
            }

            // Check if this element has calc-attr
            if let Some(calc_attr) = calc_attr_map.get(&key) {
                output.push(' ');
                output.push_str(calc_attr);
            }

            output.push('>');

            // Recurse into children
            for child in node.children() {
                serialize_node(child, calc_attr_map, output);
            }

            // Close tag
            if !is_self_closing(tag) {
                output.push_str("</");
                output.push_str(tag);
                output.push('>');
            }
        }
        scraper::Node::Text(text) => {
            output.push_str(text);
        }
        scraper::Node::Comment(comment) => {
            output.push_str("<!--");
            output.push_str(comment);
            output.push_str("-->");
        }
        scraper::Node::Doctype(_) => {
            output.push_str("<!DOCTYPE html>");
        }
        _ => {
            // Recurse into children for other node types
            for child in node.children() {
                serialize_node(child, calc_attr_map, output);
            }
        }
    }
}

fn is_self_closing(tag: &str) -> bool {
    matches!(
        tag,
        "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input"
            | "link" | "meta" | "param" | "source" | "track" | "wbr"
    )
}

/// Parse HTML and compute CSS styles for all elements.
/// Returns JSON string result.
/// - enable_js: (reserved) attempt to execute JavaScript and capture CSS variable modifications
/// - filter_properties: if Some, only return these properties (e.g., ["font-size", "color"])
#[pyo3::pyfunction]
pub fn parse_html_and_compute_styles(
    html: &str,
    enable_js: bool,
    filter_properties: Option<Vec<String>>,
) -> String {
    match compute_styles(html, enable_js, filter_properties) {
        Ok(result) => serde_json::to_string(&result).unwrap_or_else(|e| e.to_string()),
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

fn compute_styles(
    html: &str,
    enable_js: bool,
    filter_properties: Option<Vec<String>>,
) -> Result<ParseResult, String> {
    // Step 1: Parse HTML using scraper (fast) and extract CSS rules
    let scraped = scraper_adapter::parse_html_with_scraper(html).map_err(|e| e.to_string())?;

    // Step 2: Clone CSS rules for later use
    let css_rules = scraped.css_rules.clone();

    // Step 3: If JS enabled, execute JS and capture CSS variable modifications
    if enable_js {
        let js_modifications = js_executor::execute_js_and_capture_css_vars("", &HashMap::new());
        // TODO: Apply js_modifications to css_rules
        let _ = js_modifications;
    }

    // Step 4: Compute styles with filter_properties optimization
    // Pass filter_properties to cascade for early filtering
    let filter_ref = filter_properties.as_ref().map(|p| p.as_slice());
    let elements = cascade::compute_styles_from_scraper(&scraped, &css_rules, filter_ref).map_err(|e| e.to_string())?;

    Ok(ParseResult { elements })
}

/// Python module definition
#[pyo3::pymodule]
pub fn computed_css_engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_html_and_compute_styles, m)?)?;
    m.add_function(wrap_pyfunction!(parse_html_and_write_styles, m)?)?;
    Ok(())
}
