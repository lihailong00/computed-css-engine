//! Style Tree - represents the styled DOM tree

use crate::css_parser_core::{CssRule, CssOrigin};
use crate::html_parser::HtmlElement;
use std::collections::HashMap;

/// Represents an element in the style tree with all its style information
#[derive(Debug, Clone)]
pub struct StyledElement {
    pub element: HtmlElement,
    pub matched_rules: Vec<MatchedRuleRef>,
    pub cascaded_styles: HashMap<String, String>,
    pub computed_styles: HashMap<String, String>,
    pub children: Vec<StyledElement>,
}

/// Reference to a matched CSS rule
#[derive(Debug, Clone)]
pub struct MatchedRuleRef {
    pub rule: CssRule,
    pub match_position: usize,
}

/// Build a style tree from a DOM tree and CSS rules
pub fn build_style_tree(
    dom: &HtmlElement,
    css_rules: &[CssRule],
) -> Result<StyledElement, Box<dyn std::error::Error>> {
    let parent_styles = HashMap::new();
    build_styled_element(dom, css_rules, &parent_styles)
}

fn build_styled_element(
    element: &HtmlElement,
    css_rules: &[CssRule],
    parent_styles: &HashMap<String, String>,
) -> Result<StyledElement, Box<dyn std::error::Error>> {
    // Find matching rules
    let matched_rules: Vec<MatchedRuleRef> = css_rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| crate::cascade::matches_selector(element, None, &rule.selector))
        .map(|(pos, rule)| MatchedRuleRef {
            rule: rule.clone(),
            match_position: pos,
        })
        .collect();

    // Cascade styles
    let cascaded = cascade_styles(&matched_rules, parent_styles);

    // Compute final styles
    let computed = compute_final_styles(&cascaded);

    // Build children
    let children: Vec<StyledElement> = element
        .children
        .iter()
        .map(|child| build_styled_element(child, css_rules, &computed))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(StyledElement {
        element: element.clone(),
        matched_rules,
        cascaded_styles: cascaded,
        computed_styles: computed,
        children,
    })
}

/// Cascade styles from matched rules
fn cascade_styles(
    matched_rules: &[MatchedRuleRef],
    _parent_styles: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut sorted_rules = matched_rules.to_vec();
    sorted_rules.sort_by(|a, b| {
        let a_pri = get_priority(&a.rule);
        let b_pri = get_priority(&b.rule);
        b_pri.cmp(&a_pri)
    });

    let mut styles = HashMap::new();

    for rule_ref in &sorted_rules {
        for (prop, value) in &rule_ref.rule.declarations {
            expand_shorthand(prop, value, &mut styles);
        }
    }

    styles
}

fn get_priority(rule: &CssRule) -> (u32, [u32; 3]) {
    let origin_pri = match rule.origin {
        CssOrigin::UserAgent => 0,
        CssOrigin::Author => 1,
        CssOrigin::User => 2,
    };
    (origin_pri, rule.specificity)
}

/// Expand CSS shorthand properties
fn expand_shorthand(property: &str, value: &str, styles: &mut HashMap<String, String>) {
    match property {
        "margin" | "padding" => {
            let values: Vec<&str> = value.split_whitespace().collect();
            match values.len() {
                1 => {
                    styles.insert(format!("{}-top", property), values[0].to_string());
                    styles.insert(format!("{}-right", property), values[0].to_string());
                    styles.insert(format!("{}-bottom", property), values[0].to_string());
                    styles.insert(format!("{}-left", property), values[0].to_string());
                }
                2 => {
                    styles.insert(format!("{}-top", property), values[0].to_string());
                    styles.insert(format!("{}-right", property), values[1].to_string());
                    styles.insert(format!("{}-bottom", property), values[0].to_string());
                    styles.insert(format!("{}-left", property), values[1].to_string());
                }
                3 => {
                    styles.insert(format!("{}-top", property), values[0].to_string());
                    styles.insert(format!("{}-right", property), values[1].to_string());
                    styles.insert(format!("{}-bottom", property), values[2].to_string());
                    styles.insert(format!("{}-left", property), values[1].to_string());
                }
                4 => {
                    styles.insert(format!("{}-top", property), values[0].to_string());
                    styles.insert(format!("{}-right", property), values[1].to_string());
                    styles.insert(format!("{}-bottom", property), values[2].to_string());
                    styles.insert(format!("{}-left", property), values[3].to_string());
                }
                _ => {}
            }
        }
        "border" => {
            styles.insert("border-width".to_string(), value.to_string());
            styles.insert("border-style".to_string(), value.to_string());
            styles.insert("border-color".to_string(), value.to_string());
        }
        "border-width" | "border-style" | "border-color" => {
            let values: Vec<&str> = value.split_whitespace().collect();
            match values.len() {
                1 => {
                    styles.insert(format!("{}-top", property), values[0].to_string());
                    styles.insert(format!("{}-right", property), values[0].to_string());
                    styles.insert(format!("{}-bottom", property), values[0].to_string());
                    styles.insert(format!("{}-left", property), values[0].to_string());
                }
                2 => {
                    styles.insert(format!("{}-top", property), values[0].to_string());
                    styles.insert(format!("{}-right", property), values[1].to_string());
                    styles.insert(format!("{}-bottom", property), values[0].to_string());
                    styles.insert(format!("{}-left", property), values[1].to_string());
                }
                3 => {
                    styles.insert(format!("{}-top", property), values[0].to_string());
                    styles.insert(format!("{}-right", property), values[1].to_string());
                    styles.insert(format!("{}-bottom", property), values[2].to_string());
                    styles.insert(format!("{}-left", property), values[1].to_string());
                }
                4 => {
                    styles.insert(format!("{}-top", property), values[0].to_string());
                    styles.insert(format!("{}-right", property), values[1].to_string());
                    styles.insert(format!("{}-bottom", property), values[2].to_string());
                    styles.insert(format!("{}-left", property), values[3].to_string());
                }
                _ => {}
            }
        }
        "background" => {
            styles.insert("background-color".to_string(), value.to_string());
        }
        "font" => {
            styles.insert("font".to_string(), value.to_string());
        }
        _ => {
            styles.insert(property.to_string(), value.to_string());
        }
    }
}

/// Compute final styles with value resolution
fn compute_final_styles(cascaded: &HashMap<String, String>) -> HashMap<String, String> {
    let mut computed = HashMap::new();

    for (property, value) in cascaded {
        let computed_value = crate::computed::compute_value(property, value);
        computed.insert(property.clone(), computed_value);
    }

    computed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_shorthand() {
        let mut styles = HashMap::new();
        expand_shorthand("margin", "10px", &mut styles);
        assert_eq!(styles.get("margin-top"), Some(&"10px".to_string()));
        assert_eq!(styles.get("margin-right"), Some(&"10px".to_string()));
    }
}
