use std::path::PathBuf;
use html5gum::{IoReader, Tokenizer, HtmlString};
use std::fs::File;
use std::io::{self, BufReader};
use anyhow::Result;

fn to_utf8(html_string: HtmlString) -> String {
    String::from_utf8(html_string.0.clone()).unwrap()
}

pub struct Template {
    pub source: PathBuf,
}

impl Template {
    pub fn parse(&self) -> Result<()>{
        let file = File::open(&self.source)?;
        let reader = BufReader::new(file);
        for token in Tokenizer::new(IoReader::new(reader)).flatten() {
            // println!("{:?}", token);
            let s = match token {
                html5gum::Token::StartTag(tag) => {
                    let s = format!("<{}", to_utf8(tag.name));
                    // attrs todo
                    if tag.self_closing {
                        s + "/>"
                    } else {
                        s + ">"
                    }
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
        return Ok(())
    }
}
