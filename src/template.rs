#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
use anyhow::Result;
use html5gum::{HtmlString, IoReader, Tokenizer, Token, StartTag};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::TryFrom;
use std::fmt::Write;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::PathBuf;
use std::iter::Flatten;

fn to_utf8(html_string: HtmlString) -> Result<String> {
    Ok(String::from_utf8(html_string.0)?)
}

fn is_void_element(tag_name: &String) -> bool {
    matches!(
        tag_name.as_ref(),
        "area" | "base" | "br" | "col" | "embed" | "hr" |
        "img" | "input" | "link" | "meta" | "source" | "track" | "wbr"
    )
}

struct Route {
    paths: Vec<HashMap<String, String>>,
}

pub struct Template {
    sources: HashSet<String>,
    tag_stack: Vec<String>,
    parts: Vec<TemplatePart>,
}

enum TemplatePart {
    Content(String),
    HeadInjection,
    Route,
    Embed(String),
    BodyInjection,
}

impl From<&str> for TemplatePart {
    fn from(s: &str) -> Self {
        TemplatePart::Content(s.to_string())
    }
}

impl From<String> for TemplatePart {
    fn from(s: String) -> Self {
        TemplatePart::Content(s)
    }
}

impl TryFrom<HtmlString> for TemplatePart {
    type Error = anyhow::Error;
    fn try_from(html_s: HtmlString) -> Result<Self> {
        Ok(TemplatePart::Content(to_utf8(html_s)?))
    }
}

type TokenStream = Flatten<Tokenizer<IoReader<BufReader<File>>>>;
impl Template {
    pub fn compile(tokens: TokenStream) -> Result<Template> {
        let mut template = Template {
            sources: HashSet::new(),
            tag_stack: vec![],
            parts:  vec![TemplatePart::Content(String::new())],
        };
        for token in tokens{
            let new_parts = template.push_token(token)?;
            template.parts.extend(new_parts);
        }
        return Ok(template);
    }
    fn push_token(&mut self, token:Token) -> Result<Vec<TemplatePart>> {
        match token {
            Token::Doctype(_) => Ok(vec!["<DOCTYPE>".into()]),
            Token::StartTag(tag) => self.handle_start_tag(tag),
            Token::EndTag(tag) => self.handle_end_tag(tag.name),
            Token::String(html_string) => Ok(vec![html_string.try_into()?]),
            Token::Comment(_) => Ok(Vec::new()),
            Token::Error(err) => Err(anyhow::anyhow!("Error {:?}", err)),
        }
    }

    fn handle_start_tag(
        &mut self,
        tag: StartTag,
    ) -> Result<Vec<TemplatePart>> {
        let mut parts : Vec<TemplatePart>= Vec::new();
        let tag_name = to_utf8(tag.name)?;
        parts.push(format!("<{}", tag_name).into());
        if tag.attributes.len() > 0 {
            parts.push(" ".into());
        }
        for (attr_name, attr_value) in tag.attributes {
            let attr_name_string = to_utf8(attr_name)?;
            parts.push(attr_name_string.into());
            if !attr_value.is_empty() {
                parts.push(format!("=\"{}\"", to_utf8(attr_value)?).into());
            }
        }
        if tag.self_closing {
            parts.push("/".into());
        }
        parts.push(">".into());
        Ok(parts)
    }

    fn handle_end_tag(
        &mut self,
        name: HtmlString,
    ) -> Result<Vec<TemplatePart>> {
        let mut parts : Vec<TemplatePart>= Vec::new();
        let tag_name = to_utf8(name)?;
        if tag_name == "head" {
            parts.push(TemplatePart::HeadInjection);
        }
        if tag_name == "body" {
            parts.push(TemplatePart::BodyInjection);
        }
        parts.push(format!("</{}>", tag_name).into());
        Ok(parts)
    }
}

pub fn compile_template(source: &PathBuf) -> Result<Template> {
    let file = File::open(source)?;
    let reader = BufReader::new(file);
    let tokenizer = Tokenizer::new(IoReader::new(reader)).flatten();
    Template::compile(tokenizer)
}
