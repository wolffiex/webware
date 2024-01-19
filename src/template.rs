#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use html5gum::Doctype;
use html5gum::{HtmlString, IoReader, StartTag, Token, Tokenizer};
use rayon::prelude::*;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::TryFrom;
use std::fmt::Write;
use std::fs;
use std::fs::File;
use std::io::{self, BufReader};
use std::iter::Flatten;
use std::path::PathBuf;
use std::time::Instant;

use crate::cache::compute_cache_key;

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
    fn get_files(&self) -> Vec<String> {
        self.paths
            .iter()
            .filter_map(|path| path.get("file").map(|s| s.to_owned()))
            .collect()
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
        let attrs: BTreeMap<_, _> = tag.attributes
            .into_iter()
            .map(|(key, value)| Ok((to_utf8(key)?, to_utf8(value)?)))
            .collect::<Result<_>>()?;
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
                    .get("file")
                    .map(|template| Ok(vec![TemplatePart::Embed(template.to_string())]))
                    .unwrap_or_else(|| Err(anyhow::anyhow!("x-embed missing \"file\"")))
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
                "x-source" => parts.push(TemplatePart::Source(attr_value)),
                value if value.starts_with("x-") => {
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

pub struct TemplateCollection {
    preamble: String,
    directory: PathBuf,
    cache_key: u64,
    cache: HashMap<String, Template>,
}

impl TemplateCollection {
    pub fn new(directory: PathBuf) -> Self {
        let preamble = fs::read_to_string("www/preamble.js").unwrap();
        TemplateCollection {
            preamble,
            directory,
            cache_key: 0,
            cache: HashMap::new(),
        }
    }

    pub fn check(&self) -> bool {
        let new_key = compute_cache_key(&self.directory).unwrap();
        self.cache_key != new_key
    }

    pub fn recompile(&mut self) -> Result<()> {
        let new_key = compute_cache_key(&self.directory).unwrap();
        if self.cache_key != new_key {
            self.cache_key = new_key;
            self.compile_templates()?;
        }
        Ok(())
    }

    fn compile_templates(&mut self) -> Result<()> {
        let now = Instant::now(); // get current time
        let entries: Vec<_> = fs::read_dir(self.directory.clone())?
            .map(|res| res.map(|e| e.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;

        self.cache = entries
            .into_par_iter()
            .map(|path_buf| {
                let file = File::open(path_buf.clone())
                    .with_context(|| format!("Failed to open file {}", path_buf.display()))?;
                let reader = BufReader::new(file);
                let tokenizer = Tokenizer::new(IoReader::new(reader)).flatten();
                let template = Template::compile(tokenizer)?;
                let fname = path_buf
                    .strip_prefix(&self.directory)?
                    .to_path_buf()
                    .into_os_string()
                    .to_string_lossy()
                    .to_string();
                Ok((fname, template))
            })
            .collect::<Result<HashMap<String, Template>>>()?;

        let elapsed = now.elapsed(); // get elapsed time
        println!("Template compilation took {:?}", elapsed);
        Ok(())
    }

    pub fn get_page(&self, mut url_path: String) -> Result<String> {
        let mut page = Page {
            preamble: self.preamble.clone(),
            parts: Vec::new(),
            sources: HashSet::new(),
        };
        self.collect_parts(&mut url_path, "index.html".to_string(), &mut page)?;
        Ok(page.render())
    }

    fn collect_parts(
        &self,
        url_path: &mut String,
        file_name: String,
        page: &mut Page,
    ) -> Result<()> {
        let template = self
            .cache
            .get(&file_name)
            .ok_or_else(|| anyhow!("Unable to find: {}", &file_name))?;
        for part in template.parts.clone() {
            if let Some(file_name) = self.resolve_reference(url_path, &part)? {
                self.collect_parts(url_path, file_name, page)?
            } else {
                page.push_part(part);
            }
        }
        Ok(())
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
    preamble: String,
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
              {}
            </script>
        "#,
            sources_json, self.preamble,
        )
    }

    fn body_injection(&self) -> &str {
        r#"<script type="module" src="index.js"></script>"#
    }
}
