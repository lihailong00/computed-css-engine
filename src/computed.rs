//! Computed Style Calculation

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

/// Precompiled regex for parsing rgb/rgba colors
static RGB_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)").unwrap()
});

/// A computed property value with its type
#[derive(Debug, Clone)]
pub struct PropertyValue {
    pub value: String,
    pub unit: Option<String>,
}

/// Compute the final value for a CSS property
/// This handles value resolution, e.g., converting 'auto' to actual values,
/// resolving em units, computing colors, etc.
pub fn compute_value(property: &str, value: &str) -> String {
    let value = value.trim();

    // Handle 'inherit', 'initial', 'unset' - these should already be resolved in cascade
    match value {
        "inherit" | "initial" | "unset" | "revert" => return value.to_string(),
        _ => {}
    }

    // Property-specific value computation
    match property {
        // Numeric values
        "opacity" => compute_opacity(value),
        "z-index" => compute_integer(value),

        // Font weight
        "font-weight" => compute_font_weight(value),

        // Length values
        "font-size" => compute_font_size(value),
        "line-height" => compute_line_height(value),

        // Color values
        "color" => compute_color(value),
        "background-color" => compute_color(value),
        "border-color" => compute_color(value),
        "border-top-color" => compute_color(value),
        "border-right-color" => compute_color(value),
        "border-bottom-color" => compute_color(value),
        "border-left-color" => compute_color(value),
        "outline-color" => compute_color(value),

        // Spacing
        "margin" | "padding" => compute_shorthand_length(property, value),
        "border-width" => compute_shorthand_length(property, value),

        // Display
        "display" => compute_display(value),

        // Position
        "position" => value.to_string(),
        "top" | "right" | "bottom" | "left" => compute_length_or_auto(value),

        // Size
        "width" | "height" => compute_length_or_auto(value),
        "max-width" | "max-height" => compute_length_or_auto_named(value),
        "min-width" | "min-height" => compute_length_or_auto_named(value),

        // Visibility
        "visibility" => compute_visibility(value),

        // Default: return as-is
        _ => {
            // Handle calc() by keeping it as-is for now
            if value.starts_with("calc(") || value.starts_with("var(") || value.starts_with("-var(") {
                return value.to_string();
            }
            // Normalize color if it contains a color value
            if property.contains("color") {
                return compute_color(value);
            }
            value.to_string()
        }
    }
}

fn compute_opacity(value: &str) -> String {
    if let Ok(num) = value.parse::<f64>() {
        if num >= 0.0 && num <= 1.0 {
            return format!("{}", num);
        }
    }
    value.to_string()
}

fn compute_font_weight(value: &str) -> String {
    match value {
        "normal" => "400".to_string(),
        "bold" => "700".to_string(),
        "bolder" => "700".to_string(),  // simplified
        "lighter" => "400".to_string(), // simplified
        "100" | "200" | "300" | "400" | "500" | "600" | "700" | "800" | "900" => value.to_string(),
        _ => value.to_string(),
    }
}

fn compute_integer(value: &str) -> String {
    if value == "auto" {
        return value.to_string();
    }
    if let Ok(_) = value.parse::<i32>() {
        return value.to_string();
    }
    value.to_string()
}

fn compute_font_size(value: &str) -> String {
    match value {
        "xx-small" => "9px".to_string(),
        "x-small" => "10px".to_string(),
        "small" => "13px".to_string(),
        "medium" => "16px".to_string(),
        "large" => "18px".to_string(),
        "x-large" => "24px".to_string(),
        "xx-large" => "32px".to_string(),
        "smaller" => "0.8em".to_string(),
        "larger" => "1.2em".to_string(),
        _ => compute_length(value),
    }
}

fn compute_line_height(value: &str) -> String {
    if value.ends_with("%") {
        return value.to_string();
    }
    if value.parse::<f64>().is_ok() {
        return format!("{}em", value);
    }
    compute_length(value)
}

fn compute_color(value: &str) -> String {
    let named_colors: HashMap<&str, &str> = [
        ("black", "#000000"),
        ("white", "#ffffff"),
        ("red", "#ff0000"),
        ("green", "#008000"),
        ("blue", "#0000ff"),
        ("yellow", "#ffff00"),
        ("cyan", "#00ffff"),
        ("magenta", "#ff00ff"),
        ("gray", "#808080"),
        ("grey", "#808080"),
        ("orange", "#ffa500"),
        ("purple", "#800080"),
        ("pink", "#ffc0cb"),
        ("brown", "#a52a2a"),
        ("transparent", "rgba(0,0,0,0)"),
        ("currentcolor", "currentColor"),
    ].iter().cloned().collect();

    let value_lower = value.to_lowercase();

    if let Some(&hex) = named_colors.get(value_lower.as_str()) {
        return hex.to_string();
    }

    // Handle rgb() and rgba()
    if value_lower.starts_with("rgb") || value_lower.starts_with("rgba") {
        return normalize_rgb_string(value);
    }

    // Handle hex colors
    if value_lower.starts_with('#') {
        return normalize_hex_color(&value_lower);
    }

    value.to_string()
}

fn normalize_hex_color(hex: &str) -> String {
    if hex.len() == 4 {
        // #RGB -> #RRGGBB
        let r = &hex[1..2];
        let g = &hex[2..3];
        let b = &hex[3..4];
        return format!("#{}{}{}{}{}{}", r, r, g, g, b, b);
    }
    hex.to_string()
}

fn normalize_rgb_string(rgb: &str) -> String {
    // Normalize rgb(r, g, b) format
    let rgb = rgb.trim().to_lowercase();

    // Extract numbers from rgb(r, g, b) or rgba(r, g, b, a)
    if let Some(caps) = RGB_REGEX.captures(&rgb) {
        let r: u8 = caps.get(1).unwrap().as_str().parse().unwrap_or(0);
        let g: u8 = caps.get(2).unwrap().as_str().parse().unwrap_or(0);
        let b: u8 = caps.get(3).unwrap().as_str().parse().unwrap_or(0);
        return format!("rgb({}, {}, {})", r, g, b);
    }
    rgb.to_string()
}

fn compute_length(value: &str) -> String {
    let value = value.trim();

    if value.is_empty() {
        return value.to_string();
    }

    // Already have a unit
    if value.ends_with("px")
        || value.ends_with("em")
        || value.ends_with("rem")
        || value.ends_with("%")
        || value.ends_with("vh")
        || value.ends_with("vw")
        || value.ends_with("pt")
        || value.ends_with("cm")
        || value.ends_with("mm")
        || value.ends_with("in")
        || value.ends_with("ex")
        || value.ends_with("ch")
    {
        return value.to_string();
    }

    // Numeric value without unit - assume px
    if value.parse::<f64>().is_ok() {
        return format!("{}px", value);
    }

    // auto, inherit, etc.
    value.to_string()
}

fn compute_length_or_auto(value: &str) -> String {
    if value == "auto" {
        return "auto".to_string();
    }
    compute_length(value)
}

fn compute_length_or_auto_named(value: &str) -> String {
    match value {
        "auto" => "auto".to_string(),
        "none" => "none".to_string(),
        "max-content" => "max-content".to_string(),
        "min-content" => "min-content".to_string(),
        "fit-content" => "fit-content".to_string(),
        _ => compute_length(value),
    }
}

fn compute_shorthand_length(_property: &str, value: &str) -> String {
    let parts: Vec<&str> = value.split_whitespace().collect();

    if parts.len() == 1 {
        return compute_length(parts[0]);
    }

    let computed: Vec<String> = parts.iter().map(|p| compute_length(p)).collect();
    computed.join(" ")
}

fn compute_display(value: &str) -> String {
    match value {
        "inline-block" => "inline-block".to_string(),
        "inline-flex" => "inline-flex".to_string(),
        "inline-grid" => "inline-grid".to_string(),
        "inline-table" => "inline-table".to_string(),
        "table" => "table".to_string(),
        "table-row" => "table-row".to_string(),
        "table-cell" => "table-cell".to_string(),
        "table-row-group" => "table-row-group".to_string(),
        "table-header-group" => "table-header-group".to_string(),
        "table-footer-group" => "table-footer-group".to_string(),
        "flex" => "flex".to_string(),
        "grid" => "grid".to_string(),
        "flow-root" => "flow-root".to_string(),
        "contents" => "contents".to_string(),
        _ => value.to_string(),
    }
}

fn compute_visibility(value: &str) -> String {
    match value {
        "visible" => "visible".to_string(),
        "hidden" => "hidden".to_string(),
        "collapse" => "collapse".to_string(),
        _ => value.to_string(),
    }
}

/// Parse a CSS numeric value into a number and unit
pub fn parse_numeric_value(value: &str) -> Option<(f64, Option<String>)> {
    let value = value.trim();

    let mut end = 0;
    let mut has_dot = false;

    for (i, c) in value.char_indices() {
        match c {
            '.' if !has_dot => {
                has_dot = true;
                end = i + 1;
            }
            c if c.is_ascii_digit() => {
                end = i + 1;
            }
            _ => break,
        }
    }

    if end == 0 {
        return None;
    }

    let number: f64 = value[..end].parse().ok()?;
    let unit = value[end..].trim().to_string();

    if unit.is_empty() {
        Some((number, None))
    } else {
        Some((number, Some(unit)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_numeric_value() {
        assert_eq!(parse_numeric_value("10px"), Some((10.0, Some("px".to_string()))));
        assert_eq!(parse_numeric_value("1.5em"), Some((1.5, Some("em".to_string()))));
        assert_eq!(parse_numeric_value("100"), Some((100.0, None)));
    }

    #[test]
    fn test_compute_color() {
        assert_eq!(compute_color("red"), "#ff0000");
        assert_eq!(compute_color("rgb(0,0,0)"), "rgb(0, 0, 0)");
        assert_eq!(compute_color("#fff"), "#ffffff");
    }

    #[test]
    fn test_compute_display() {
        assert_eq!(compute_display("flex"), "flex");
        assert_eq!(compute_display("inline-flex"), "inline-flex");
    }
}
