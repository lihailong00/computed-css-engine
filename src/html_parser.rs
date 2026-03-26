//! HTML Parser module - simple recursive descent parser with proper UTF-8 handling

use std::collections::HashMap;

/// Represents a parsed HTML element
#[derive(Debug, Clone)]
pub struct HtmlElement {
    pub tag_name: String,
    pub attributes: HashMap<String, String>,
    pub children: Vec<HtmlElement>,
    pub text_content: Option<String>,  // For style tags and script tags
}

impl HtmlElement {
    pub fn new(tag_name: String) -> Self {
        HtmlElement {
            tag_name,
            attributes: HashMap::new(),
            children: Vec::new(),
            text_content: None,
        }
    }
}

/// Parse HTML string into a tree of HtmlElements
pub fn parse_html(html: &str) -> Result<HtmlElement, Box<dyn std::error::Error>> {
    let mut parser = HtmlTreeParser::new(html);
    parser.parse()
}

/// Simple HTML tree parser with proper UTF-8 handling
struct HtmlTreeParser<'a> {
    html: &'a str,
    char_indices: Vec<(usize, char)>,
    pos: usize,
}

impl<'a> HtmlTreeParser<'a> {
    fn new(html: &'a str) -> Self {
        let char_indices: Vec<(usize, char)> = html.char_indices().map(|(i, c)| (i, c)).collect();
        HtmlTreeParser {
            html,
            char_indices,
            pos: 0,
        }
    }

    fn parse(&mut self) -> Result<HtmlElement, Box<dyn std::error::Error>> {
        self.skip_doctype();
        self.skip_whitespace();
        self.parse_element()
    }

    fn current_char(&self) -> Option<char> {
        self.char_indices.get(self.pos).map(|(_, c)| *c)
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.current_char();
        self.pos += 1;
        c
    }

    fn skip_doctype(&mut self) {
        if self.rest_starts_with("<!DOCTYPE") {
            for _ in 0..9 {
                self.advance();
            }
            while let Some(c) = self.current_char() {
                if c == '>' {
                    self.advance();
                    break;
                }
                self.advance();
            }
        }
    }

    fn rest_starts_with(&self, s: &str) -> bool {
        self.substring_from_pos().starts_with(s)
    }

    fn substring_from_pos(&self) -> String {
        self.char_indices[self.pos..].iter().map(|(_, c)| *c).collect()
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current_char() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn parse_element(&mut self) -> Result<HtmlElement, Box<dyn std::error::Error>> {
        self.skip_whitespace();

        if self.current_char() != Some('<') {
            return Err("Expected '<'".into());
        }
        self.advance();

        let tag_name = self.parse_tag_name()?;
        let attrs = self.parse_attributes()?;

        let self_closing = if self.current_char() == Some('/') {
            self.advance();
            true
        } else {
            is_self_closing(&tag_name)
        };

        if self.current_char() == Some('>') {
            self.advance();
        }

        let mut children = Vec::new();

        if !self_closing {
            children = self.parse_children(&tag_name)?;
        }

        Ok(HtmlElement {
            tag_name,
            attributes: attrs,
            children,
            text_content: None,
        })
    }

    fn parse_tag_name(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        self.skip_whitespace();
        let start = self.pos;
        while let Some(c) = self.current_char() {
            if c.is_whitespace() || c == '>' || c == '/' {
                break;
            }
            self.advance();
        }
        let end = self.pos;

        let start_byte = self.char_indices[start].0;
        let end_byte = if end < self.char_indices.len() {
            self.char_indices[end].0
        } else {
            self.html.len()
        };

        Ok(self.html[start_byte..end_byte].to_lowercase())
    }

    fn parse_attributes(&mut self) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
        let mut attrs = HashMap::new();
        loop {
            self.skip_whitespace();
            match self.current_char() {
                None | Some('>') => break,
                Some('/') if self.substring_from_pos().starts_with("/>") => break,
                Some('<') if self.rest_starts_with("<!--") => break,
                _ => {}
            }

            let name = self.parse_attribute_name()?;
            self.skip_whitespace();
            if self.current_char() == Some('=') {
                self.advance();
                self.skip_whitespace();
                let value = self.parse_attribute_value()?;
                attrs.insert(name, value);
            } else if !name.is_empty() {
                attrs.insert(name, String::new());
            }
        }
        Ok(attrs)
    }

    fn parse_attribute_name(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let start = self.pos;
        while let Some(c) = self.current_char() {
            if c.is_whitespace() || c == '=' || c == '>' {
                break;
            }
            self.advance();
        }
        let end = self.pos;

        let start_byte = self.char_indices[start].0;
        let end_byte = if end < self.char_indices.len() {
            self.char_indices[end].0
        } else {
            self.html.len()
        };
        Ok(self.html[start_byte..end_byte].to_string())
    }

    fn parse_attribute_value(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let quote = self.current_char().filter(|&c| c == '"' || c == '\'');

        if quote.is_some() {
            self.advance();
        }

        let start = self.pos;
        let mut end = self.pos;
        while let Some(c) = self.current_char() {
            if quote.is_some() && c == quote.unwrap() {
                end = self.pos;
                break;
            }
            if quote.is_none() && (c.is_whitespace() || c == '>') {
                end = self.pos;
                break;
            }
            end = self.pos;
            self.advance();
        }

        let start_byte = self.char_indices[start].0;
        let end_byte = if end < self.char_indices.len() {
            self.char_indices[end].0
        } else {
            self.html.len()
        };
        Ok(self.html[start_byte..end_byte].to_string())
    }

    fn parse_children(&mut self, parent_tag: &str) -> Result<Vec<HtmlElement>, Box<dyn std::error::Error>> {
        let mut children = Vec::new();
        let mut text_buffer = String::new();

        loop {
            self.skip_whitespace();
            if self.current_char().is_none() {
                break;
            }

            if self.rest_starts_with("</") {
                // Save any accumulated text as a text node
                if !text_buffer.trim().is_empty() && (parent_tag == "style" || parent_tag == "script") {
                    // For style/script, create a pseudo-element with text content
                    let mut text_elem = HtmlElement::new(format!("_text_{}", parent_tag));
                    text_elem.text_content = Some(text_buffer.trim().to_string());
                    children.push(text_elem);
                    text_buffer.clear();
                }
                self.advance();
                self.advance();
                let closing_tag = self.parse_tag_name()?;
                self.skip_whitespace();
                if self.current_char() == Some('>') {
                    self.advance();
                    if closing_tag == parent_tag {
                        break;
                    }
                }
                continue;
            }

            if self.rest_starts_with("<!--") {
                self.skip_comment();
                continue;
            }

            if self.current_char() == Some('<') {
                // Save any accumulated text first
                if !text_buffer.trim().is_empty() {
                    let mut text_elem = HtmlElement::new("_text".to_string());
                    text_elem.text_content = Some(text_buffer.trim().to_string());
                    children.push(text_elem);
                    text_buffer.clear();
                }

                let child = self.parse_element()?;
                children.push(child);
            } else {
                // Accumulate text content
                while self.current_char().is_some() && self.current_char() != Some('<') {
                    if let Some(c) = self.current_char() {
                        text_buffer.push(c);
                    }
                    self.advance();
                }
            }
        }
        Ok(children)
    }

    fn skip_comment(&mut self) {
        for _ in 0..4 {
            self.advance();
        }
        while let Some(_c) = self.current_char() {
            if self.rest_starts_with("-->") {
                for _ in 0..3 {
                    self.advance();
                }
                break;
            }
            self.advance();
        }
    }
}

fn is_self_closing(tag: &str) -> bool {
    matches!(tag, "br" | "hr" | "img" | "input" | "meta" | "link" | "area" | "base" | "col" | "embed" | "param" | "source" | "track" | "wbr")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_html() {
        let html = "<div class=\"container\"><p>Hello</p></div>";
        let result = parse_html(html);
        assert!(result.is_ok());
        let root = result.unwrap();
        assert_eq!(root.tag_name, "div");
        assert_eq!(root.attributes.get("class"), Some(&"container".to_string()));
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].tag_name, "p");
    }

    #[test]
    fn test_parse_with_attributes() {
        let html = r#"<div id="main" class="container" style="color:red"><p>Text</p></div>"#;
        let result = parse_html(html);
        assert!(result.is_ok());
        let root = result.unwrap();
        assert_eq!(root.attributes.get("id"), Some(&"main".to_string()));
        assert_eq!(root.attributes.get("class"), Some(&"container".to_string()));
        assert_eq!(root.attributes.get("style"), Some(&"color:red".to_string()));
    }

    #[test]
    fn test_utf8_content() {
        let html = "<div>日本語テスト</div>";
        let result = parse_html(html);
        assert!(result.is_ok());
        let root = result.unwrap();
        assert_eq!(root.tag_name, "div");
    }
}
