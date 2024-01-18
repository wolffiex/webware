#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
use anyhow::Context;
use anyhow::Result;
use html5gum::Doctype;
use html5gum::{HtmlString, IoReader, StartTag, Token, Tokenizer};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::TryFrom;
use std::fmt::Write;
use std::fs::File;
use std::io::{self, BufReader};
use std::iter::Flatten;
use std::path::PathBuf;

use crate::cache::DirectoryCache;

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

#[derive(Debug, Clone)]
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
    tag_stack: Vec<String>,
    parts: Vec<TemplatePart>,
    partial_route: Option<Route>,
}

#[derive(Debug, Clone)]
enum TemplatePart {
    Content(String),
    HeadInjection,
    Route(Route),
    Embed(String),
    BodyInjection,
    Source(String),
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
            tag_stack: vec![],
            parts: vec![TemplatePart::Content(String::new())],
            partial_route: None,
        };
        for token in tokens {
            let new_parts = template.push_token(token)?;
            template.parts.extend(new_parts);
        }
        Ok(template)
    }
    fn push_token(&mut self, token: Token) -> Result<Vec<TemplatePart>> {
        match token {
            Token::Doctype(doc_type) => self.handle_doctype(doc_type),
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
        if !attributes.is_empty() {
            parts.push(" ".into());
        }
        for (attr_name, attr_value) in attributes {
            match attr_name.as_str() {
                ":source" => parts.push(TemplatePart::Source(attr_value)),
                value if value.starts_with(':') => {
                    println!("ATR {} {}", attr_name, attr_value);
                }
                _ => {
                    parts.push(attr_name.into());
                    if !attr_value.is_empty() {
                        parts.push(format!("=\"{}\"", attr_value).into());
                    }
                }
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

        if !tag_name.starts_with("x-") {
            parts.push(format!("</{}>", tag_name).into());
        }
        self.pop_tag(&tag_name);
        Ok(parts)
    }

    fn handle_doctype(&self, doc_type: Doctype) -> Result<Vec<TemplatePart>> {
        let doc_string = format!("<!DOCTYPE {}>", to_utf8(doc_type.name)?);
        Ok(vec![doc_string.into()])
    }
}

struct TemplateCollection {
    directory: PathBuf,
    cache: DirectoryCache<Template>,
}

impl TemplateCollection {
    fn compile_template(&self, file_name: &String) -> Result<Template> {
        let mut path = self.directory.clone();
        path.push(file_name);
        let file = File::open(path.clone())
            .with_context(|| format!("Failed to open file {}", path.display()))?;
        let reader = BufReader::new(file);
        let tokenizer = Tokenizer::new(IoReader::new(reader)).flatten();
        Template::compile(tokenizer)
    }

    fn get_template(&mut self, file_name: String) -> &Template {
        self.cache.get_or_insert(file_name.clone(), || {
            let mut path = self.directory.clone();
            println!("Cache miss {}", file_name);
            path.push(file_name);
            let file = File::open(path.clone()).unwrap();
            let reader = BufReader::new(file);
            let tokenizer = Tokenizer::new(IoReader::new(reader)).flatten();
            Template::compile(tokenizer).unwrap()
        })
    }

    pub fn get_page(&mut self, url_path: &mut String) -> String {
        let mut page = Page {
            parts: Vec::new(),
            sources: HashSet::new(),
        };
        self.collect_parts(url_path, "index.html".to_string(), &mut page);
        page.render()
    }

    fn collect_parts(&mut self, url_path: &mut String, file_name: String, page: &mut Page) {
        let template = self.get_template(file_name);
        for part in template.parts.clone() {
            if let Some(file_name) = self.resolve_reference(url_path, &part).unwrap() {
                self.collect_parts(url_path, file_name, page)
            } else {
                page.push_part(part);
            }
        }
    }

    fn resolve_reference(
        &self,
        url_path: &mut String,
        part: &TemplatePart,
    ) -> Result<Option<String>> {
        match part {
            TemplatePart::Embed(file_name) => Ok(Some(file_name.to_string())),
            TemplatePart::Route(route) => route.match_path(url_path).map(|s| Some(s.to_owned())),
            _ => Ok(None),
        }
    }
}

struct Page {
    parts: Vec<TemplatePart>,
    sources: HashSet<String>,
}

impl Page {
    fn push_part(&mut self, part: TemplatePart) {
        if let TemplatePart::Source(name) = part {
            self.sources.insert(name.to_string());
        } else {
            self.parts.push(part);
        }
    }

    fn render(&self) -> String {
        let mut html = String::new();
        for part in &self.parts {
            match part {
                TemplatePart::Content(s) => html.push_str(&s),
                TemplatePart::HeadInjection => html.push_str(&self.head_injection()),
                TemplatePart::BodyInjection => html.push_str(&self.body_injection()),
                _ => unreachable!(),
            }
        }
        html
    }

    fn head_injection(&self) -> String {
        let sources_json = serde_json::to_string(&self.sources).unwrap_or("[]".into());
        format!(
            r#"
            <script>
              const sources = {}
              console.log(sources)
              const queryParams = sources.map((str, index) => 
                  `source=${{encodeURIComponent(str)}}`).join('&');
              const eventSource = new EventSource('/api?' + queryParams);
              let streamRunning = true
              const eventBuffer = [];
              eventSource.addEventListener('stream_stop', () => {{
                streamRunning = false
                eventSource.close();
              }});

              let resolver = null
              eventSource.onmessage = e => {{
                eventBuffer.push(e);
                const currentResolver = resolver
                resolver = null
                if (currentResolver) currentResolver()
              }};
              window.apiEventSource = async function*() {{
                while (streamRunning || eventBuffer.length) {{
                  if (eventBuffer.length > 0) {{
                    yield eventBuffer.shift();
                  }} else {{
                    await new Promise((resolve, _) => {{
                      resolver = resolve
                    }});
                  }}
                }}
              }}
            </script>
        "#,
            sources_json
        )
    }

    fn body_injection(&self) -> &str {
        r#"<script type="module" src="index.js"></script>"#
    }
}
