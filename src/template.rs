use anyhow::Result;
use html5gum::{HtmlString, IoReader, Tokenizer};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::PathBuf;

fn to_utf8(html_string: HtmlString) -> Result<String> {
    Ok(String::from_utf8(html_string.0.clone())?)
}

pub struct Template {
    pub source: PathBuf,
}

pub struct StringTemplatePart {
    content: String,
}

impl StringTemplatePart {
    pub fn new() -> StringTemplatePart {
        StringTemplatePart {
            content: String::new(),
        }
    }

    fn push_str(&mut self, html_string: HtmlString) -> Result<()> {
        let s = to_utf8(html_string)?;
        Ok(self.content.push_str(&s))
    }

    pub fn write_start_tag(
        &mut self,
        name: HtmlString,
        attributes: BTreeMap<HtmlString, HtmlString>,
        self_closing: bool,
    ) -> Result<()> {
        write!(self.content, "<{}", to_utf8(name)?)?;
        for (tag_name, tag_value) in attributes {
            // let tag_value_string = to_utf8(tag_value)?;
            if tag_value.is_empty() {
                self.push_str(tag_name)?;
            } else {
                write!(self.content, "{}=\"{}\"", to_utf8(tag_name)?, to_utf8(tag_value)?)?;
            }
        }
        if self_closing {
            self.content.push_str("/");
        }
        Ok(self.content.push_str(">"))
    }

    pub fn write_end_tag(
        &mut self,
        name: HtmlString,
    ) -> Result<()> {
        Ok(write!(self.content, "</{}>", to_utf8(name)?)?)
    }
}

impl Template {
    pub fn parse(&self) -> Result<String> {
        let file = File::open(&self.source)?;
        let reader = BufReader::new(file);
        let mut part = StringTemplatePart::new();
        for token in Tokenizer::new(IoReader::new(reader)).flatten() {
            // println!("{:?}", token);
            match token {
                html5gum::Token::StartTag(tag) => {
                    part.write_start_tag(tag.name, tag.attributes, tag.self_closing)?
                }
                html5gum::Token::EndTag(tag) => part.write_end_tag(tag.name)?,
                html5gum::Token::String(html_string) => part.push_str(html_string)?,
                html5gum::Token::Doctype(_) => part.push_str(HtmlString("<DOCTYPE>".as_bytes().to_vec()))?,
                html5gum::Token::Comment(_) => (),
                html5gum::Token::Error(err) => {
                    panic!("Error {:?}", err)
                }
            };
        }
        return Ok(part.content);
    }
}
