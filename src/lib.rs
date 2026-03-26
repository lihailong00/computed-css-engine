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
fn write_styles_to_html_attr(
    html: &str,
    elements: &[ElementStyles],
    filter_properties: Option<&[String]>,
) -> String {
    use scraper::{Html, Selector};

    let document = Html::parse_document(html);

    // Build a map from element index to its computed styles
    // We need to match elements by their position in the DOM
    let target_props: Vec<String> = if let Some(props) = filter_properties {
        props.iter().map(|s| s.to_string()).collect()
    } else {
        vec!["font-size".to_string(), "font-weight".to_string(), "color".to_string(), "display".to_string()]
    };

    // For each element with styles, find it in the DOM and add calc-attr
    let mut result = html.to_string();

    for elem in elements {
        if elem.computed_styles.is_empty() {
            continue;
        }

        // Build calc-attr value
        let mut attrs: Vec<String> = Vec::new();
        for prop in &target_props {
            if let Some(value) = elem.computed_styles.get(prop) {
                if !value.is_empty() {
                    attrs.push(format!("{}: {}", prop, value));
                }
            }
        }

        if attrs.is_empty() {
            continue;
        }

        // Filter out display: none as it's obvious
        attrs.retain(|attr| !attr.contains("display: none"));

        let calc_attr = format!("calc-attr=\"{}\"", attrs.join("; "));

        // Find the element in the DOM and insert the attribute
        // We use the tag and attributes to identify the element
        let tag = &elem.tag;
        let selector_str = build_selector_for_element(tag, &elem.attributes);

        let selector = match Selector::parse(&selector_str) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for elem_in_dom in document.select(&selector) {
            // Get the element's outer HTML and modify it
            let outer_html = elem_in_dom.html();
            if let Some(modified) = insert_attribute(&outer_html, &calc_attr) {
                result = result.replace(&outer_html, &modified);
                break;
            }
        }
    }

    result
}

/// Build a CSS selector from tag and attributes
fn build_selector_for_element(tag: &str, attrs: &HashMap<String, String>) -> String {
    let mut selector = tag.to_string();

    if let Some(id) = attrs.get("id") {
        selector = format!("{}#{}", selector, id);
    } else if let Some(class) = attrs.get("class") {
        // Use first class only
        let first_class = class.split_whitespace().next().unwrap_or("");
        if !first_class.is_empty() {
            selector = format!("{}.{}", selector, first_class);
        }
    }

    selector
}

/// Insert an attribute into an element's opening tag
fn insert_attribute(element_html: &str, new_attr: &str) -> Option<String> {
    // Find the closing > of the opening tag
    if let Some(pos) = element_html.find('>') {
        let mut opening_tag = element_html[..pos + 1].to_string();
        let rest = &element_html[pos + 1..];

        // Check if calc-attr already exists
        if opening_tag.contains("calc-attr=") {
            // Replace existing calc-attr
            let re = regex::Regex::new(r#"calc-attr="[^"]*""#).unwrap();
            opening_tag = re.replace(&opening_tag, new_attr.trim_start_matches("calc-attr=")).to_string();
        } else {
            // Insert before the closing >
            opening_tag = opening_tag.replace(">", &format!(" {}", new_attr));
            opening_tag.push('>');
        }

        return Some(format!("{}{}", opening_tag, rest));
    }
    None
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
pub fn css_parser(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_html_and_compute_styles, m)?)?;
    m.add_function(wrap_pyfunction!(parse_html_and_write_styles, m)?)?;
    Ok(())
}
