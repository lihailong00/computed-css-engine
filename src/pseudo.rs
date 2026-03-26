//! Pseudo Classes and Pseudo Elements support

use std::collections::HashMap;

/// Represents a pseudo selector
#[derive(Debug, Clone)]
pub enum PseudoSelector {
    /// Pseudo class (e.g., :hover, :focus, :first-child)
    PseudoClass(PseudoClass),
    /// Pseudo element (e.g., ::before, ::after)
    PseudoElement(PseudoElement),
}

/// Known pseudo classes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PseudoClass {
    Hover,
    Focus,
    Active,
    Visited,
    Link,
    FirstChild,
    LastChild,
    NthChild(u32),
    NthLastChild(u32),
    FirstOfType,
    LastOfType,
    NthOfType(u32),
    NthLastOfType(u32),
    OnlyChild,
    OnlyOfType,
    Empty,
    Not(String),
    Custom(String),
}

/// Known pseudo elements
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PseudoElement {
    Before,
    After,
    FirstLine,
    FirstLetter,
    Selection,
    Custom(String),
}

/// Parse a pseudo selector string
pub fn parse_pseudo(pseudo_str: &str) -> Option<PseudoSelector> {
    let pseudo_str = pseudo_str.trim();

    if pseudo_str.starts_with("::") {
        let name = &pseudo_str[2..];
        return Some(PseudoSelector::PseudoElement(parse_pseudo_element(name)));
    }

    if pseudo_str.starts_with(':') {
        let name = &pseudo_str[1..];
        return Some(PseudoSelector::PseudoClass(parse_pseudo_class(name)));
    }

    None
}

fn parse_pseudo_element(name: &str) -> PseudoElement {
    match name.to_lowercase().as_str() {
        "before" => PseudoElement::Before,
        "after" => PseudoElement::After,
        "first-line" => PseudoElement::FirstLine,
        "first-letter" => PseudoElement::FirstLetter,
        "selection" => PseudoElement::Selection,
        _ => PseudoElement::Custom(name.to_string()),
    }
}

fn parse_pseudo_class(name: &str) -> PseudoClass {
    let name_lower = name.to_lowercase();

    if let Some(paren_pos) = name_lower.find('(') {
        let func_name = &name[..paren_pos];
        let arg = name[paren_pos + 1..].trim_end_matches(')');

        match func_name {
            "nth-child" => {
                if arg == "even" {
                    return PseudoClass::NthChild(2);
                }
                if arg == "odd" {
                    return PseudoClass::NthChild(1);
                }
                if let Ok(n) = arg.parse() {
                    return PseudoClass::NthChild(n);
                }
            }
            "nth-last-child" => {
                if let Ok(n) = arg.parse() {
                    return PseudoClass::NthLastChild(n);
                }
            }
            "nth-of-type" => {
                if let Ok(n) = arg.parse() {
                    return PseudoClass::NthOfType(n);
                }
            }
            "nth-last-of-type" => {
                if let Ok(n) = arg.parse() {
                    return PseudoClass::NthLastOfType(n);
                }
            }
            "not" => {
                return PseudoClass::Not(arg.to_string());
            }
            _ => {}
        }
    }

    match name_lower.as_str() {
        "hover" => PseudoClass::Hover,
        "focus" => PseudoClass::Focus,
        "active" => PseudoClass::Active,
        "visited" => PseudoClass::Visited,
        "link" => PseudoClass::Link,
        "first-child" => PseudoClass::FirstChild,
        "last-child" => PseudoClass::LastChild,
        "first-of-type" => PseudoClass::FirstOfType,
        "last-of-type" => PseudoClass::LastOfType,
        "only-child" => PseudoClass::OnlyChild,
        "only-of-type" => PseudoClass::OnlyOfType,
        "empty" => PseudoClass::Empty,
        _ => PseudoClass::Custom(name.to_string()),
    }
}

/// Extract pseudo selectors from a rule's selector
pub fn extract_pseudo_selectors(selector: &str) -> Vec<PseudoSelector> {
    let mut pseudos = Vec::new();

    let mut current = String::new();
    let mut in_pseudo = false;

    for c in selector.chars() {
        if c == ':' && !in_pseudo {
            in_pseudo = true;
            current.push(c);
        } else if in_pseudo && (c.is_alphanumeric() || c == '-' || c == '(' || c == ')' || c == '_') {
            current.push(c);
        } else if in_pseudo {
            if let Some(pseudo) = parse_pseudo(&current) {
                pseudos.push(pseudo);
            }
            in_pseudo = false;
            current.clear();
        }
    }

    if in_pseudo {
        if let Some(pseudo) = parse_pseudo(&current) {
            pseudos.push(pseudo);
        }
    }

    pseudos
}

/// Separate pseudo element styles from regular declarations
pub fn separate_pseudo_styles(declarations: HashMap<String, String>) -> (HashMap<String, String>, HashMap<String, HashMap<String, String>>) {
    let mut regular = HashMap::new();
    let mut pseudo_styles: HashMap<String, HashMap<String, String>> = HashMap::new();

    for (property, value) in declarations {
        if let Some((pseudo, prop)) = property.split_once(' ') {
            if pseudo.starts_with("::") || pseudo.starts_with(':') {
                pseudo_styles.entry(pseudo.to_string())
                    .or_insert_with(HashMap::new)
                    .insert(prop.to_string(), value);
                continue;
            }
        }
        regular.insert(property, value);
    }

    (regular, pseudo_styles)
}

/// Check if a property belongs to a pseudo element
pub fn is_pseudo_property(property: &str) -> bool {
    property.starts_with("::") || property.starts_with(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pseudo_class() {
        assert_eq!(parse_pseudo_class("hover"), PseudoClass::Hover);
        assert_eq!(parse_pseudo_class("first-child"), PseudoClass::FirstChild);
        assert_eq!(parse_pseudo_class("nth-child(2)"), PseudoClass::NthChild(2));
    }

    #[test]
    fn test_parse_pseudo_element() {
        assert_eq!(parse_pseudo_element("before"), PseudoElement::Before);
        assert_eq!(parse_pseudo_element("after"), PseudoElement::After);
    }

    #[test]
    fn test_extract_pseudo_selectors() {
        let pseudos = extract_pseudo_selectors("div:hover::before");
        assert_eq!(pseudos.len(), 2);
    }
}
