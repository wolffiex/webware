use anyhow::Result;
use html5gum::{HtmlString, IoReader, Tokenizer};
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
    pub fn parse(&self) -> Result<()> {
        let file = File::open(&self.source)?;
        let reader = BufReader::new(file);
        for token in Tokenizer::new(IoReader::new(reader)).flatten() {
            // println!("{:?}", token);
            let s = match token {
                html5gum::Token::StartTag(tag) => {
                    let mut s = format!("<{}", to_utf8(tag.name));
                    let attr_string: String = tag
                        .attributes
                        .into_iter()
                        .map(|(tag_name, tag_value)| {
                            let tag_name_string = to_utf8(tag_name);
                            let tag_value_string = to_utf8(tag_value);
                            if tag_value_string.is_empty() {
                                tag_name_string
                            } else {
                                format!("{}=\"{}\"", tag_name_string, tag_value_string)
                            }
                        })
                        .collect::<Vec<_>>() // First, collect the mapped strings into a Vec
                        .join(" "); // Then, join them with spaces
                    if !attr_string.is_empty() {
                        s.push(' ');
                        s.push_str(&attr_string);
                    }
                    if tag.self_closing {
                        s.push_str("/")
                    }
                    s.push_str("/>");
                    s
                }
                html5gum::Token::EndTag(tag) => format!("</{}>", to_utf8(tag.name)),
                html5gum::Token::String(html_string) => to_utf8(html_string),
                html5gum::Token::Comment(_) => "".to_string(),
                html5gum::Token::Doctype(_) => "<DOCTYPE>".to_string(),
                html5gum::Token::Error(err) => {
                    panic!("Error {:?}", err)
                }
            };
            print!("{}", s);
        }
        return Ok(());
    }
}
