#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
use anyhow::Result;
use html5gum::{HtmlString, IoReader, StartTag, Token, Tokenizer};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::TryFrom;
use std::fmt::Write;
use std::fs::File;
use std::io::{self, BufReader};
use std::iter::Flatten;
use std::path::PathBuf;

fn to_utf8(html_string: HtmlString) -> Result<String> {
    Ok(String::from_utf8(html_string.0)?)
}

fn is_void_element(tag_name: &String) -> bool {
    matches!(
        tag_name.as_ref(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "source"
            | "track"
            | "wbr"
            | "x-path"
            | "x-embed"
    )
}

#[derive(Debug)]
struct Route {
    paths: Vec<BTreeMap<String, String>>,
}

pub struct Template {
    sources: HashSet<String>,
    tag_stack: Vec<String>,
    parts: Vec<TemplatePart>,
    partial_route: Option<Route>,
}

#[derive(Debug)]
enum TemplatePart {
    Content(String),
    HeadInjection,
    Route(Route),
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
            parts: vec![TemplatePart::Content(String::new())],
            partial_route: None,
        };
        for token in tokens {
            let new_parts = template.push_token(token)?;
            template.parts.extend(new_parts);
        }
        return Ok(template);
    }
    fn push_token(&mut self, token: Token) -> Result<Vec<TemplatePart>> {
        match token {
            Token::Doctype(_) => Ok(vec!["<DOCTYPE>".into()]),
            Token::StartTag(tag) => self.handle_start_tag(tag),
            Token::EndTag(tag) => self.handle_end_tag(tag.name),
            Token::String(html_string) => Ok(vec![html_string.try_into()?]),
            Token::Comment(_) => Ok(Vec::new()),
            Token::Error(err) => Err(anyhow::anyhow!("Error {:?}", err)),
        }
    }

    fn push_tag(&mut self, tag_name: &String) {
        self.tag_stack.push(tag_name.to_string());
        // println!("{:?}", self.tag_stack);
    }

    fn pop_tag(&mut self, tag_name: &String) {
        let expected_tag = self.tag_stack.pop().expect("Tag stack is empty");
        // if &expected_tag != tag_name{
        //     for s in self.parts.iter().rev().take(8) {
        //         println!("{:?}", s);
        //     }
        // }
        assert_eq!(&expected_tag, tag_name);
        // println!("{:?}", self.tag_stack);
        if expected_tag.as_str() == "x-route" {
            self.parts.push(TemplatePart::Route(self.partial_route.take().unwrap()));
        }
    }

    fn handle_start_tag(&mut self, tag: StartTag) -> Result<Vec<TemplatePart>> {
        let tag_name = to_utf8(tag.name)?;
        self.push_tag(&tag_name);
        let mut attrs = BTreeMap::new();
        for (key, value) in tag.attributes.into_iter() {
            attrs.insert(to_utf8(key)?, to_utf8(value)?);
        }
        let result: Result<Vec<TemplatePart>> = match tag_name.as_str() {
            "x-route" => {
                assert!(self.partial_route.is_none());
                self.partial_route = Some(Route { paths: Vec::new() });
                Ok(Vec::new())
            }
            "x-path" => {
                println!("pEIANT {:?}", &attrs);
                self.partial_route
                    .as_mut()
                    .map(|route| route.paths.push(attrs))
                    .ok_or(anyhow::anyhow!("Found path outside of route"))?;
                Ok(Vec::new())
            }
            "x-embed" => {
                println!("EMERDB {:?}", &attrs);
                attrs
                    .get("template")
                    .map(|template| Ok(vec![TemplatePart::Embed(template.to_string())]))
                    .unwrap_or_else(|| Err(anyhow::anyhow!("x-embed missing \"template\"")))
            }
            _ => Ok(self.convert_tag(&tag_name, attrs, tag.self_closing)?),
        };
        if tag.self_closing || is_void_element(&tag_name) {
            self.pop_tag(&tag_name);
        }
        result
    }

    fn convert_tag(
        &mut self,
        tag_name: &String,
        attributes: BTreeMap<String, String>,
        self_closing: bool,
    ) -> Result<Vec<TemplatePart>> {
        let mut parts: Vec<TemplatePart> = Vec::new();
        parts.push(format!("<{}", tag_name).into());
        if attributes.len() > 0 {
            parts.push(" ".into());
        }
        for (attr_name, attr_value) in attributes {
            parts.push(attr_name.into());
            if !attr_value.is_empty() {
                parts.push(format!("=\"{}\"", attr_value).into());
            }
        }
        if self_closing {
            parts.push("/".into());
        }
        parts.push(">".into());
        Ok(parts)
    }

    fn handle_end_tag(&mut self, name: HtmlString) -> Result<Vec<TemplatePart>> {
        let mut parts: Vec<TemplatePart> = Vec::new();
        let tag_name = to_utf8(name)?;
        if tag_name == "head" {
            parts.push(TemplatePart::HeadInjection);
        }
        if tag_name == "body" {
            parts.push(TemplatePart::BodyInjection);
        }
        parts.push(format!("</{}>", tag_name).into());
        self.pop_tag(&tag_name);
        Ok(parts)
    }
}

pub fn compile_template(source: &PathBuf) -> Result<Template> {
    let file = File::open(source)?;
    let reader = BufReader::new(file);
    let tokenizer = Tokenizer::new(IoReader::new(reader)).flatten();
    Template::compile(tokenizer)
}

struct TemplateCollection {
    directory: PathBuf,
}

impl TemplateCollection {
    pub fn render(&self, route: &String) {
        let index_html = "index.html".to_string();
        self.render_template(route, self.compile_template(&index_html).unwrap());
    }

    pub fn compile_template(&self, file_name: &String) -> Result<Template> {
        let mut path = self.directory.clone();
        path.push(file_name);
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let tokenizer = Tokenizer::new(IoReader::new(reader)).flatten();
        Template::compile(tokenizer)
    }

    fn render_template(&self, route: &String, template:Template) {
    }
}

struct Page {
    content : String
}
