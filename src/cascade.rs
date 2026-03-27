//! Cascade and Inheritance Engine

use crate::css_parser_core::{CssOrigin, CssRule, get_user_agent_stylesheet};
use crate::html_parser::HtmlElement;
use crate::computed::compute_value;
use crate::scraper_adapter::{ScrapedElement, ScrapedElements};
use std::collections::HashMap;

/// Indexed CSS rules for fast matching
/// Groups rules by selector type to avoid checking all rules for each element
#[derive(Debug, Clone)]
struct IndexedCssRules<'a> {
    /// Rules that match any element (universal selector *)
    universal: Vec<&'a CssRule>,
    /// Rules indexed by tag name (lowercase)
    by_tag: HashMap<String, Vec<&'a CssRule>>,
    /// Rules indexed by class name
    by_class: HashMap<String, Vec<&'a CssRule>>,
    /// Rules indexed by ID
    by_id: HashMap<String, Vec<&'a CssRule>>,
    /// Rules with attribute selectors
    attr: Vec<&'a CssRule>,
    /// Rules with pseudo selectors
    pseudo: Vec<&'a CssRule>,
    /// Rules with complex selectors (descendant, child, etc.) - must check tree structure
    complex: Vec<&'a CssRule>,
}

impl<'a> IndexedCssRules<'a> {
    fn new(rules: &'a [&'a CssRule]) -> Self {
        let mut indexed = IndexedCssRules {
            universal: Vec::new(),
            by_tag: HashMap::new(),
            by_class: HashMap::new(),
            by_id: HashMap::new(),
            attr: Vec::new(),
            pseudo: Vec::new(),
            complex: Vec::new(),
        };

        for rule in rules {
            let selector = rule.selector.trim();
            if selector.is_empty() {
                continue;
            }

            let first_char = selector.chars().next().unwrap_or(' ');

            // Check for combinators (space, >, +, ~) - these need tree structure
            if selector.contains(' ') || selector.starts_with('>')
                || selector.starts_with('+') || selector.starts_with('~') {
                indexed.complex.push(rule);
                continue;
            }

            match first_char {
                '*' => {
                    indexed.universal.push(rule);
                }
                '#' => {
                    // ID selector
                    let id = selector.strip_prefix('#').unwrap_or("").to_lowercase();
                    indexed.by_id.entry(id).or_default().push(rule);
                }
                '.' => {
                    // Class selector
                    let class = selector.strip_prefix('.').unwrap_or("").to_lowercase();
                    indexed.by_class.entry(class).or_default().push(rule);
                }
                '[' => {
                    // Attribute selector
                    indexed.attr.push(rule);
                }
                ':' => {
                    // Pseudo selector
                    indexed.pseudo.push(rule);
                }
                _ => {
                    // Tag selector
                    let tag_lower = selector.to_lowercase();
                    indexed.by_tag.entry(tag_lower).or_default().push(rule);
                }
            }
        }

        indexed
    }
}

/// Compute styles using ScrapedElements (from scraper parser)
/// filter_properties: if Some, only compute these properties (e.g., ["font-size", "color"])
pub fn compute_styles_from_scraper(
    scraped: &ScrapedElements,
    css_rules: &[CssRule],
    filter_properties: Option<&[String]>,
) -> Result<Vec<crate::ElementStyles>, Box<dyn std::error::Error>> {
    // Get user-agent rules (cached reference)
    let ua_rules = get_user_agent_stylesheet();

    // Combine user-agent rules with author rules (user-agent first, then author overrides)
    let mut all_rules: Vec<&CssRule> = Vec::with_capacity(ua_rules.len() + css_rules.len());
    all_rules.extend(ua_rules.iter());
    all_rules.extend(css_rules.iter());

    // Filter CSS rules to only those that contain any of the target properties
    let filtered_rules: Vec<&CssRule> = if let Some(props) = filter_properties {
        filter_rules_by_properties(&all_rules, props)
    } else {
        all_rules
    };

    // Create indexed rules for fast matching
    let indexed_rules = IndexedCssRules::new(&filtered_rules);

    // Extract CSS variables
    let css_variables = extract_css_variables_from_rules(&filtered_rules);

    // Compute styles in DOM order (elements are already in DOM order)
    // Use a stack-based approach to track parent styles for em -> px conversion
    let mut computed_styles_list: Vec<HashMap<String, String>> = Vec::with_capacity(scraped.elements.len());
    // Use indices instead of references to avoid clone
    let mut parent_stack: Vec<usize> = Vec::new();

    for (idx, element) in scraped.elements.iter().enumerate() {
        // Pop stack entries that are at or above current depth
        while parent_stack.len() > element.depth {
            parent_stack.pop();
        }

        // Get parent's computed styles using index into computed_styles_list
        let parent_computed: Option<&HashMap<String, String>> =
            if parent_stack.is_empty() { None } else { computed_styles_list.get(*parent_stack.last().unwrap()) };

        let computed = compute_styles_for_scraper_element(
            element,
            &indexed_rules,
            &css_variables,
            filter_properties,
            parent_computed,
        );

        computed_styles_list.push(computed);

        // Push current element's index to stack for its children
        parent_stack.push(idx);
    }

    // Build final result - move from computed_styles_list instead of cloning
    let elements = scraped.elements.iter().zip(computed_styles_list.into_iter()).map(|(element, computed_styles)| {
        crate::ElementStyles {
            path: element.tag.clone(),
            tag: element.tag.clone(),
            attributes: element.attributes.clone(),
            matched_rules: Vec::new(),
            computed_styles,
        }
    }).collect();

    Ok(elements)
}

/// Filter CSS rules to only those that contain any of the target properties
fn filter_rules_by_properties<'a>(rules: &'a [&'a CssRule], target_props: &[String]) -> Vec<&'a CssRule> {
    rules.iter()
        .filter(|rule| {
            rule.declarations.keys().any(|prop| {
                target_props.iter().any(|tp| prop == tp)
            })
        })
        .copied()
        .collect()
}

/// Compute styles for a single ScrapedElement using indexed rules
fn compute_styles_for_scraper_element<'a>(
    element: &ScrapedElement,
    indexed_rules: &IndexedCssRules<'a>,
    css_variables: &HashMap<String, String>,
    filter_properties: Option<&[String]>,
    parent_computed: Option<&HashMap<String, String>>,
) -> HashMap<String, String> {
    let mut computed = HashMap::new();

    // First, apply inherited properties from parent
    if let Some(parent) = parent_computed {
        for (prop, value) in parent {
            if is_inherited(prop) {
                computed.insert(prop.clone(), value.clone());
            }
        }
    }

    // Find matching rules using indexed lookup
    let matched_rules = find_matching_rules_for_scraper_indexed(element, indexed_rules);

    // Cascade
    let mut cascaded: HashMap<String, CascadedValue> = HashMap::new();

    for rule in &matched_rules {
        for (property, value) in &rule.declarations {
            let has_important = value.contains("!important");
            let clean_value = value.replace("!important", "").trim().to_string();

            let cascaded_value = CascadedValue {
                value: clean_value,
                specificity: rule.specificity,
                origin: rule.origin,
                important: has_important,
            };

            if should_override(&cascaded_value, cascaded.get(property)) {
                cascaded.insert(property.clone(), cascaded_value);
            }
        }
    }

    // Apply inline style (highest priority)
    if let Some(ref inline) = element.inline_style {
        for part in inline.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some(colon_pos) = part.find(':') {
                let property = part[..colon_pos].trim().to_lowercase();
                let value = part[colon_pos + 1..].trim().to_string();
                cascaded.insert(property, CascadedValue {
                    value,
                    specificity: [0, 0, 0], // Inline styles have highest specificity in practice
                    origin: CssOrigin::Author,
                    important: false,
                });
            }
        }
    }

    // Resolve values - only for properties in filter list
    let empty_hashmap = HashMap::new();
    let parent_ref: &HashMap<String, String> = parent_computed.unwrap_or(&empty_hashmap);

    for (property, cascaded_value) in cascaded {
        // Skip properties not in filter list
        if let Some(props) = filter_properties {
            if !props.iter().any(|p| p == &property) {
                continue;
            }
        }

        let resolved = resolve_value(
            &cascaded_value.value,
            &property,
            parent_ref,
            css_variables,
            &computed,
        );
        let computed_val = compute_value_with_parent_font_size(&property, &resolved, parent_computed);
        computed.insert(property, computed_val);
    }

    computed
}

/// Find matching rules using indexed lookup
fn find_matching_rules_for_scraper_indexed<'a>(
    element: &ScrapedElement,
    indexed: &IndexedCssRules<'a>,
) -> Vec<&'a CssRule> {
    let mut matched = Vec::new();

    // Check universal rules (*) - these need full matching for pseudo-classes
    for rule in &indexed.universal {
        if scraper_element_matches_selector(element, &rule.selector) {
            matched.push(*rule);
        }
    }

    // Check tag rules - direct match since we indexed by exact tag name
    let tag_lower = element.tag.to_lowercase();
    if let Some(tag_rules) = indexed.by_tag.get(&tag_lower) {
        matched.extend(tag_rules.iter().copied());
    }

    // Check ID rules - direct match since we indexed by exact ID
    if let Some(ref id) = element.id {
        let id_lower = id.to_lowercase();
        if let Some(id_rules) = indexed.by_id.get(&id_lower) {
            matched.extend(id_rules.iter().copied());
        }
    }

    // Check class rules - direct match since we indexed by exact class name
    if let Some(ref class) = element.class {
        for class_name in class.split_whitespace() {
            let class_lower = class_name.to_lowercase();
            if let Some(class_rules) = indexed.by_class.get(&class_lower) {
                matched.extend(class_rules.iter().copied());
            }
        }
    }

    // Check attribute rules - need full matching
    for rule in &indexed.attr {
        if scraper_element_matches_selector(element, &rule.selector) {
            matched.push(*rule);
        }
    }

    // Check pseudo rules - need full matching
    for rule in &indexed.pseudo {
        if scraper_element_matches_selector(element, &rule.selector) {
            matched.push(*rule);
        }
    }

    // Check complex rules (descendant, child, etc.) - need full matching
    for rule in &indexed.complex {
        if scraper_element_matches_selector(element, &rule.selector) {
            matched.push(*rule);
        }
    }

    matched
}

/// Find matching rules for a ScrapedElement
fn find_matching_rules_for_scraper<'a>(element: &ScrapedElement, rules: &'a [&'a CssRule]) -> Vec<&'a CssRule> {
    let mut matched = Vec::new();

    for rule in rules {
        if scraper_element_matches_selector(element, &rule.selector) {
            matched.push(*rule);
        }
    }

    matched
}

/// Check if a ScrapedElement matches a CSS selector
fn scraper_element_matches_selector(element: &ScrapedElement, selector: &str) -> bool {
    let selector = selector.trim();
    if selector.is_empty() {
        return false;
    }

    // Handle comma-separated selectors
    for part in selector.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if matches_simple_selector_for_scraper(element, part) {
            return true;
        }
    }

    false
}

/// Match a simple selector against a ScrapedElement
fn matches_simple_selector_for_scraper(element: &ScrapedElement, selector: &str) -> bool {
    let first_char = selector.chars().next().unwrap_or(' ');

    match first_char {
        '*' => true,
        '#' => {
            let id = selector.strip_prefix('#').unwrap_or("");
            element.id.as_deref() == Some(id)
        }
        '.' => {
            let class = selector.strip_prefix('.').unwrap_or("");
            element.class.as_ref().map_or(false, |c| {
                c.split_whitespace().any(|cl| cl == class)
            })
        }
        ':' => {
            // Pseudo selectors - accept common ones
            matches!(
                selector,
                ":root" | ":first-child" | ":last-child" | ":only-child" |
                ":first-of-type" | ":last-of-type" | ":only-of-type" |
                ":empty" | ":hover" | ":focus" | ":active" | ":visited" | ":link"
            ) || selector.contains(":nth-child") || selector.contains(":not(")
        }
        '[' => {
            // Attribute selector
            if selector.ends_with(']') {
                let inner = &selector[1..selector.len()-1];
                if let Some(eq_pos) = inner.find('=') {
                    let attr_name = inner[..eq_pos].trim();
                    let attr_value = inner[eq_pos + 1..].trim().trim_matches('"').trim_matches('\'');
                    element.attributes.get(attr_name).map_or(false, |v| v == attr_value)
                } else {
                    element.attributes.contains_key(inner)
                }
            } else {
                false
            }
        }
        _ => {
            // Tag selector
            let tag_part: String = selector.chars().take_while(|c| !": #[.".contains(*c)).collect();
            tag_part.is_empty() || tag_part == "*" || element.tag == tag_part.to_lowercase()
        }
    }
}

/// Compute styles for all elements in the DOM
pub fn compute_element_styles(
    dom: &HtmlElement,
    css_rules: &[CssRule],
) -> Result<Vec<crate::ElementStyles>, Box<dyn std::error::Error>> {
    // Extract CSS custom properties (variables) from :root or html
    let css_variables = extract_css_variables(dom, css_rules);

    let mut elements = Vec::new();
    let parent_computed = HashMap::new();

    compute_styles_for_element(dom, css_rules, &mut elements, &parent_computed, None, &css_variables)?;
    Ok(elements)
}

/// Extract CSS variables from rules
fn extract_css_variables_from_rules<'a>(css_rules: &'a [&'a CssRule]) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    for rule in css_rules {
        if rule.selector.contains(":root") || rule.selector == "html" || rule.selector == ":root" {
            for (prop, value) in &rule.declarations {
                if prop.starts_with("--") {
                    vars.insert(prop.clone(), value.clone());
                }
            }
        }
    }

    vars
}

/// Extract CSS custom properties (variables) from the root element
fn extract_css_variables(_dom: &HtmlElement, css_rules: &[CssRule]) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    // Find rules that apply to :root or html
    for rule in css_rules {
        if rule.selector.contains(":root") || rule.selector == "html" || rule.selector == ":root" {
            for (prop, value) in &rule.declarations {
                if prop.starts_with("--") {
                    vars.insert(prop.clone(), value.clone());
                }
            }
        }
    }

    vars
}

fn compute_styles_for_element(
    element: &HtmlElement,
    css_rules: &[CssRule],
    elements: &mut Vec<crate::ElementStyles>,
    parent_computed: &HashMap<String, String>,
    parent_element: Option<&HtmlElement>,
    css_variables: &HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Find matching rules for this element, using parent info for combinators
    let matched_rules = find_matching_rules(element, parent_element, css_rules);

    // Compute cascade
    let mut cascaded: HashMap<String, CascadedValue> = HashMap::new();

    for rule in &matched_rules {
        for (property, value) in &rule.declarations {
            let has_important = value.contains("!important");
            let clean_value = value.replace("!important", "").trim().to_string();

            let cascaded_value = CascadedValue {
                value: clean_value,
                specificity: rule.specificity,
                origin: rule.origin,
                important: has_important,
            };

            if should_override(&cascaded_value, cascaded.get(property)) {
                cascaded.insert(property.clone(), cascaded_value);
            }
        }
    }

    // Apply inheritance from parent
    let mut computed = HashMap::new();
    for (property, value) in parent_computed {
        if is_inherited(property) {
            computed.insert(property.clone(), value.clone());
        }
    }

    // Apply cascaded values (resolving inherit/initial/var())
    for (property, cascaded_value) in cascaded {
        let resolved = resolve_value(&cascaded_value.value, &property, parent_computed, css_variables, &computed);
        computed.insert(property, resolved);
    }

    // Convert to final computed styles
    let mut final_computed = HashMap::new();
    for (property, value) in &computed {
        let computed_val = compute_value(property, value);
        final_computed.insert(property.clone(), computed_val);
    }

    // Generate path for this element
    let path = generate_element_path(element);

    // Get attributes
    let attributes: HashMap<String, String> = element.attributes.clone();

    elements.push(crate::ElementStyles {
        path,
        tag: element.tag_name.clone(),
        attributes,
        matched_rules: matched_rules.iter().map(|r| crate::MatchedRule {
            selector: r.selector.clone(),
            specificity: r.specificity,
            origin: format!("{:?}", r.origin).to_lowercase(),
            declarations: r.declarations.clone(),
        }).collect(),
        computed_styles: final_computed.clone(),
    });

    // Process children
    let child_parent_computed = computed;
    for child in &element.children {
        // Skip text content pseudo-elements
        if child.tag_name.starts_with("_text") {
            continue;
        }
        compute_styles_for_element(child, css_rules, elements, &child_parent_computed, Some(element), css_variables)?;
    }

    Ok(())
}

/// Resolve a CSS value: handle inherit, initial, var()
fn resolve_value(
    value: &str,
    property: &str,
    parent_computed: &HashMap<String, String>,
    css_variables: &HashMap<String, String>,
    _computed: &HashMap<String, String>,
) -> String {
    let value = value.trim().to_lowercase();

    if value == "inherit" {
        return parent_computed.get(property).cloned().unwrap_or_else(|| "inherit".to_string());
    }

    if value == "initial" {
        return get_initial_value(property);
    }

    if value == "unset" {
        if is_inherited(property) {
            return parent_computed.get(property).cloned().unwrap_or_else(|| get_initial_value(property));
        } else {
            return get_initial_value(property);
        }
    }

    // Handle CSS variable var()
    if value.starts_with("var(") {
        return resolve_css_var(&value, css_variables);
    }

    value.to_string()
}

fn resolve_css_var(value: &str, css_variables: &HashMap<String, String>) -> String {
    // Extract variable name from var(--name) or var(--name, fallback)
    let inner = value.trim_start_matches("var(").trim_end_matches(")");

    // Get the variable name
    let parts: Vec<&str> = inner.split(',').collect();
    let var_name = parts[0].trim();

    // Get the fallback if provided
    let fallback = if parts.len() > 1 { Some(parts[1].trim()) } else { None };

    if let Some(var_value) = css_variables.get(var_name) {
        return var_value.clone();
    }

    fallback.map(|s| s.to_string()).unwrap_or_else(|| "".to_string())
}

fn get_initial_value(property: &str) -> String {
    match property {
        "display" => "inline".to_string(),
        "position" => "static".to_string(),
        "top" | "right" | "bottom" | "left" => "auto".to_string(),
        "width" | "height" => "auto".to_string(),
        "color" => "black".to_string(),
        "background-color" => "transparent".to_string(),
        "font-size" => "16px".to_string(),  // Default font-size is 16px (medium)
        "font-family" => "serif".to_string(),
        "font-weight" => "normal".to_string(),
        "font-style" => "normal".to_string(),
        "line-height" => "normal".to_string(),
        "text-align" => "start".to_string(),
        "text-decoration" => "none".to_string(),
        "text-transform" => "none".to_string(),
        "visibility" => "visible".to_string(),
        "opacity" => "1".to_string(),
        "border-style" => "none".to_string(),
        "border-width" => "medium".to_string(),
        "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => "0".to_string(),
        "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => "0".to_string(),
        "z-index" => "auto".to_string(),
        "overflow" => "visible".to_string(),
        "float" => "none".to_string(),
        "clear" => "none".to_string(),
        "cursor" => "auto".to_string(),
        "direction" => "ltr".to_string(),
        _ => "initial".to_string(),
    }
}

/// Compute value with em -> px conversion using parent's font-size
fn compute_value_with_parent_font_size(
    property: &str,
    value: &str,
    parent_computed: Option<&HashMap<String, String>>,
) -> String {
    // Handle font-size with em units
    if property == "font-size" {
        let value = value.trim();
        if value.ends_with("em") {
            // Parse the em value
            if let Ok(em_value) = value.trim_end_matches("em").parse::<f64>() {
                // Get parent's font-size, default to 16px
                let parent_font_size = parent_computed
                    .and_then(|p| p.get("font-size"))
                    .and_then(|v| parse_px_value(v))
                    .unwrap_or(16.0);

                let px_value = em_value * parent_font_size;
                return format!("{}px", px_value);
            }
        }
    }

    // For other properties, use standard compute_value
    compute_value(property, value)
}

/// Parse a px value to f64, returns None if not a valid px value
fn parse_px_value(value: &str) -> Option<f64> {
    let value = value.trim().to_lowercase();
    if value.ends_with("px") {
        let num_str = &value[..value.len() - 2];
        num_str.trim().parse::<f64>().ok()
    } else if let Ok(num) = value.parse::<f64>() {
        // Unitless number, assume px
        Some(num)
    } else {
        None
    }
}

struct CascadedValue {
    value: String,
    specificity: [u32; 3],
    origin: CssOrigin,
    important: bool,
}

fn should_override(new: &CascadedValue, existing: Option<&CascadedValue>) -> bool {
    match existing {
        None => true,
        Some(existing) => {
            // !important always wins
            if new.important && !existing.important {
                return true;
            }
            if !new.important && existing.important {
                return false;
            }

            let new_origin_priority = get_origin_priority(&new.origin);
            let existing_origin_priority = get_origin_priority(&existing.origin);

            if new_origin_priority != existing_origin_priority {
                return new_origin_priority > existing_origin_priority;
            }

            // Compare specificity lexicographically: [ids, classes, elements]
            if new.specificity[0] != existing.specificity[0] {
                return new.specificity[0] > existing.specificity[0];
            }
            if new.specificity[1] != existing.specificity[1] {
                return new.specificity[1] > existing.specificity[1];
            }
            if new.specificity[2] != existing.specificity[2] {
                return new.specificity[2] > existing.specificity[2];
            }

            false
        }
    }
}

fn get_origin_priority(origin: &CssOrigin) -> u32 {
    match origin {
        CssOrigin::UserAgent => 1,
        CssOrigin::Author => 2,
        CssOrigin::User => 3,
    }
}

fn find_matching_rules<'a>(element: &HtmlElement, parent_element: Option<&HtmlElement>, rules: &'a [CssRule]) -> Vec<&'a CssRule> {
    rules.iter().filter(|rule| matches_selector(element, parent_element, &rule.selector)).collect()
}

/// Check if an element matches a CSS selector
pub fn matches_selector(element: &HtmlElement, parent: Option<&HtmlElement>, selector: &str) -> bool {
    let selector = selector.trim();

    if selector.is_empty() {
        return true;
    }

    // Fast path: skip based on selector prefix and element attributes
    let first_char = selector.chars().next().unwrap_or(' ');
    match first_char {
        '#' if !element.attributes.contains_key("id") => return false,
        '.' if !element.attributes.contains_key("class") => return false,
        '[' => {} // attribute selector, check normally
        ':' => {} // pseudo selector, check normally
        '*' => {} // universal
        ' ' | '>' | '+' | '~' => {} // combinator
        _ => {
            // Could be tag name - check if it matches
            let tag_part: String = selector.chars().take_while(|c| !": #[.".contains(*c)).collect();
            if !tag_part.is_empty() && tag_part != "*" && element.tag_name != tag_part {
                return false;
            }
        }
    }

    // Handle comma-separated selectors
    for part in selector.split(',') {
        if matches_simple_selector(element, parent, part.trim()) {
            return true;
        }
    }

    false
}

fn matches_simple_selector(element: &HtmlElement, parent: Option<&HtmlElement>, selector: &str) -> bool {
    let selector = selector.trim();

    if selector.is_empty() {
        return true;
    }

    // Handle descendant combinator (space) - ancestor descendant
    // e.g., ".all *" means element or ancestor matches ".all" and element matches "*"
    if selector.contains(' ') {
        let parts: Vec<&str> = selector.split_whitespace().filter(|s| !s.is_empty()).collect();
        if parts.len() >= 2 {
            // Last part is what we need to match, rest are ancestors
            let ancestor_part = parts[..parts.len() - 1].join(" ");
            let target_part = parts[parts.len() - 1];

            // Check if any ancestor matches ancestor_part
            let mut current_parent = parent;
            let mut ancestor_matches = ancestor_part.trim().is_empty();

            while let Some(p) = current_parent {
                if matches_simple_selector(p, None, ancestor_part.trim()) {
                    ancestor_matches = true;
                    break;
                }
                // Go up one more level - but we don't have grandparent, so we stop
                current_parent = None;
            }

            // If ancestor_part is just descendant combinators, any ancestor matches
            if ancestor_part.trim().is_empty() {
                ancestor_matches = true;
            }

            return ancestor_matches && matches_simple_selector(element, None, target_part);
        }
    }

    // Handle child combinator (>) - parent > child
    // e.g., ".all > *" means parent matches ".all" and element matches "*"
    if selector.starts_with('>') {
        let target = selector.trim_start_matches('>').trim();
        if target.is_empty() {
            return false;
        }
        // Check if direct parent matches the ancestor part (what comes before >)
        // But we only have one > here, so the parent should match the rest
        // Actually, this case is for "> *", meaning match parent and then *
        // For proper child combinator, we need to parse "ancestor > child"
        let target_parts: Vec<&str> = target.split('>').collect();
        if target_parts.len() == 2 {
            let ancestor = target_parts[0].trim();
            let child = target_parts[1].trim();
            if parent.map_or(false, |p| matches_simple_selector(p, None, ancestor)) {
                return matches_simple_selector(element, None, child);
            }
            return false;
        }
        return matches_simple_selector(element, None, target);
    }

    // Handle :root pseudo-class
    if selector == ":root" && element.tag_name == "html" {
        return true;
    }

    let s = selector;

    // Universal selector
    if s == "*" {
        return true;
    }

    // ID selector
    if let Some(id) = s.strip_prefix('#') {
        return element.attributes.get("id").map_or(false, |v| v == id);
    }

    // Class selector
    if let Some(class) = s.strip_prefix('.') {
        return element.attributes.get("class").map_or(false, |v| {
            v.split_whitespace().any(|c| c == class)
        });
    }

    // Attribute selector
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len()-1];
        if let Some(eq_pos) = inner.find('=') {
            let attr_name = inner[..eq_pos].trim();
            let attr_value = inner[eq_pos + 1..].trim().trim_matches('"').trim_matches('\'');
            return element.attributes.get(attr_name).map_or(false, |v| v == attr_value);
        } else {
            return element.attributes.contains_key(inner);
        }
    }

    // Element selector
    let element_part: String = s.chars().take_while(|c| !": #[.".contains(*c)).collect();

    if !element_part.is_empty() {
        if element_part != "*" && element.tag_name != element_part {
            return false;
        }

        let remaining = &s[element_part.len()..];
        if !remaining.is_empty() && remaining != "*" {
            return matches_pseudo_selectors(element, remaining);
        }

        return true;
    }

    // Starts with : (pseudo class/element only)
    if s.starts_with(':') {
        return matches_pseudo_selectors(element, s);
    }

    false
}

fn matches_pseudo_selectors(_element: &HtmlElement, selector: &str) -> bool {
    // Only accept static structural pseudo-classes that can be evaluated at render time
    // DO NOT accept :hover, :focus, :active, :visited, :link - these require user interaction
    matches!(
        selector,
        ":root" | ":first-child" | ":last-child" | ":only-child" |
        ":first-of-type" | ":last-of-type" | ":only-of-type" |
        ":empty"
    ) || selector.contains(":nth-child") || selector.contains(":not(")
}

fn is_inherited(property: &str) -> bool {
    matches!(
        property,
        "color" |
        "font-family" |
        "font-size" |
        "font-style" |
        "font-weight" |
        "font-variant" |
        "line-height" |
        "letter-spacing" |
        "text-align" |
        "text-indent" |
        "text-transform" |
        "visibility" |
        "white-space" |
        "word-spacing" |
        "direction" |
        "quotes" |
        "cursor" |
        "opacity" |
        "border-collapse" |
        "caption-side" |
        "empty-cells" |
        "list-style-type" |
        "list-style-position" |
        "list-style-image" |
        "orphans" |
        "widows"
    )
}

fn generate_element_path(element: &HtmlElement) -> String {
    element.tag_name.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_inherited() {
        assert!(is_inherited("color"));
        assert!(is_inherited("font-size"));
        assert!(is_inherited("visibility"));
        assert!(!is_inherited("display"));
        assert!(!is_inherited("margin"));
        assert!(!is_inherited("padding"));
    }

    #[test]
    fn test_matches_selector() {
        let div = HtmlElement {
            tag_name: "div".to_string(),
            attributes: HashMap::new(),
            children: vec![],
            text_content: None,
        };

        let p_with_class = HtmlElement {
            tag_name: "p".to_string(),
            attributes: [("class".to_string(), "text".to_string())].into_iter().collect(),
            children: vec![],
            text_content: None,
        };

        let html_elem = HtmlElement {
            tag_name: "html".to_string(),
            attributes: HashMap::new(),
            children: vec![],
            text_content: None,
        };

        assert!(matches_selector(&div, None, "div"));
        assert!(matches_selector(&p_with_class, None, "p"));
        assert!(matches_selector(&p_with_class, None, ".text"));
        assert!(!matches_selector(&div, None, "span"));
        assert!(matches_selector(&html_elem, None, ":root"));
    }

    #[test]
    fn test_resolve_css_var() {
        let mut vars = HashMap::new();
        vars.insert("--primary".to_string(), "red".to_string());

        assert_eq!(resolve_css_var("var(--primary)", &vars), "red");
        assert_eq!(resolve_css_var("var(--secondary, blue)", &vars), "blue");
        assert_eq!(resolve_css_var("var(--undefined, green)", &vars), "green");
    }
}
