use anyhow::Result;
use html5gum::{HtmlString, IoReader, Tokenizer};
use std::fmt::Write; // Import the Write trait which provides the write! macro for Strings.
use std::fs::File;
use std::io::{self, BufReader};
use std::path::PathBuf;

fn to_utf8(html_string: HtmlString) -> String {
    String::from_utf8(html_string.0.clone()).unwrap()
}

pub struct Template {
    pub source: PathBuf,
}

impl Template {
    pub fn parse(&self) -> Result<String> {
        let file = File::open(&self.source)?;
        let reader = BufReader::new(file);
        let mut result = String::new();
        for token in Tokenizer::new(IoReader::new(reader)).flatten() {
            // println!("{:?}", token);
            match token {
                html5gum::Token::StartTag(tag) => {
                    write!(result, "<{}", to_utf8(tag.name))?;
                    if tag.attributes.len() > 0 {
                        write!(result, " ")?;
                    }
                    for (tag_name, tag_value) in tag.attributes {
                        let tag_name_string = to_utf8(tag_name);
                        let tag_value_string = to_utf8(tag_value);
                        if tag_value_string.is_empty() {
                            result.push_str(&tag_name_string);
                        } else {
                            write!(result, "{}=\"{}\"", tag_name_string, tag_value_string)?;
                        }
                    }
                    if tag.self_closing {
                        write!(result, "/")?;
                    }
                    write!(result, ">")?;
                },
                html5gum::Token::EndTag(tag) => write!(result, "</{}>", to_utf8(tag.name))?,
                html5gum::Token::String(html_string) => result.push_str(&to_utf8(html_string)),
                html5gum::Token::Comment(_) => (),
                html5gum::Token::Doctype(_) => write!(result, "<DOCTYPE>")?,
                html5gum::Token::Error(err) => {
                    panic!("Error {:?}", err)
                }
            };
        }
        return Ok(result);
    }
}
