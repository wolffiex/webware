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

impl Route {
    fn match_path(&self, url_path: &mut String) -> Result<String> {
        assert_eq!('/', url_path.remove(0));

        // find the index of the next slash
        let index = url_path.find('/').unwrap_or(url_path.len());
        let path_part: String = url_path.drain(..index).collect();
        for path in &self.paths {
            if Some(&path_part) == path.get("url") {
                return Ok(path.get("file").expect("No file for path.").to_owned());
            }
        }
        Err(anyhow::anyhow!("No match for path {}", url_path))
    }
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
            self.parts
                .push(TemplatePart::Route(self.partial_route.take().unwrap()));
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

    pub fn compile_template(&self, file_name: &String) -> Result<Template> {
        let mut path = self.directory.clone();
        path.push(file_name);
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let tokenizer = Tokenizer::new(IoReader::new(reader)).flatten();
        Template::compile(tokenizer)
    }

    fn get_index(&self) -> Result<Template> {
        let index_html = "index.html".to_string();
        self.compile_template(&index_html)
    }
}

struct Page {
    html: String,
    sources: HashSet<String>,
}

impl Page {
    fn build(url_path: String, collection: &TemplateCollection) -> Result<String> {
        let index_template = collection.get_index()?;
        let mut page = Page {
            html: String::new(),
            sources: HashSet::new(),
        };
        page.collect_sources(&mut url_path.clone(), &index_template, collection)?;
        Ok(page.html)
    }

    fn collect_sources(
        &mut self,
        url_path: &mut String,
        template: &Template,
        collection: &TemplateCollection,
    ) -> Result<()> {
        self.sources.extend(template.sources.clone());
        for part in &template.parts {
            if let Some(file_name) = Page::resolve_reference(url_path, &part)? {
                let template = collection.compile_template(&file_name)?;
                self.collect_sources(url_path, &template, collection)?;
            }
        }
        Ok(())
    }

    fn inject_head(&mut self) -> Result<()>{
        Ok(())
    }

    fn inject_body(&mut self) -> Result<()> {
        Ok(())
    }

    fn process_template(
        &mut self,
        url_path: &mut String,
        template_file_name: &String,
        collection: &TemplateCollection,
    ) -> Result<()> {
        let template = collection.compile_template(template_file_name)?;
        for part in &template.parts {
            if let Some(file_name) = Page::resolve_reference(url_path, part)? {
                self.process_template(url_path, &file_name, collection)?;
            } else {
                match part {
                    TemplatePart::Content(s) => self.html.push_str(s),
                    TemplatePart::HeadInjection => self.inject_head()?,
                    TemplatePart::BodyInjection => self.inject_body()?,
                    _ => unreachable!(),
                };
            }
        }
        Ok(())
    }

    fn resolve_reference(url_path: &mut String, part: &TemplatePart) -> Result<Option<String>> {
        match part {
            TemplatePart::Embed(file_name) => Ok(Some(file_name.to_string())),
            TemplatePart::Route(route) => route.match_path(url_path).map(|s| Some(s.to_owned())),
            _ => Ok(None),
        }
    }
}
