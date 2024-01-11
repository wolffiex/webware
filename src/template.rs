use anyhow::Result;
use html5gum::{HtmlString, IoReader, Tokenizer, Token, StartTag};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::PathBuf;

fn to_utf8(html_string: HtmlString) -> Result<String> {
    Ok(String::from_utf8(html_string.0.clone())?)
}

fn is_void_element(tag_name: &String) -> bool {
    matches!(
        tag_name.as_ref(),
        "area" | "base" | "br" | "col" | "embed" | "hr" |
        "img" | "input" | "link" | "meta" | "source" | "track" | "wbr"
    )
}

pub struct Template {
    pub content: String,
}

impl Template {
    pub fn new() -> Template {
        Template {
            content: String::new(),
        }
    }
    pub fn push_token(&mut self, token:Token) -> Result<()> {
        match token {
            Token::StartTag(tag) => {
                self.handle_start_tag(tag)
            }
            Token::EndTag(tag) => self.write_end_tag(tag.name),
            Token::String(html_string) => self.push_str(html_string),
            Token::Doctype(_) => self.push_str(HtmlString("<DOCTYPE>".as_bytes().to_vec())),
            Token::Comment(_) => Ok(()),
            Token::Error(err) => {
                panic!("Error {:?}", err)
            }
        }
    }

    fn push_str(&mut self, html_string: HtmlString) -> Result<()> {
        let s = to_utf8(html_string)?;
        Ok(self.content.push_str(&s))
    }

    pub fn handle_start_tag(
        &mut self,
        tag: StartTag,
    ) -> Result<()> {
        let tag_name = to_utf8(tag.name)?;
        write!(self.content, "<{}", tag_name)?;
        if tag.attributes.len() > 0 {
            self.content.push_str(" ");
        }
        for (attr_name, attr_value) in tag.attributes {
            let attr_name_string = self.push_str(attr_name)?;
            if attr_value.is_empty() {
            } else {
                write!(self.content, "=\"{}\"", to_utf8(attr_value)?)?;
            }
        }
        if tag.self_closing {
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

pub fn compile_template(source: &PathBuf) -> Result<Template> {
    let file = File::open(source)?;
    let reader = BufReader::new(file);
    let mut template = Template::new();
    for token in Tokenizer::new(IoReader::new(reader)).flatten() {
        template.push_token(token)?;
    }
    return Ok(template);
}
