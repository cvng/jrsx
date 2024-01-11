use proc_macro::TokenStream;
use quote::quote;
use regex::Regex;
use std::collections::HashSet;
use std::env;
use std::fs;
use syn::meta::ParseNestedMeta;
use syn::parse::Result;
use syn::parse_macro_input;
use syn::DeriveInput;
use syn::LitStr;

// TODO: https://crates.io/crates/syn-rsx
const COMPONENT_RE: &str = r#"<([A-Z][a-zA-Z0-9]*)\s*([^>/]*)\s*/*?>"#;

#[derive(Default)]
struct TemplateAttributes {
    path: Option<LitStr>,
}

impl TemplateAttributes {
    fn parse(&mut self, meta: ParseNestedMeta) -> Result<()> {
        if meta.path.is_ident("path") {
            self.path = meta.value()?.parse()?;
            Ok(())
        } else {
            Err(meta.error("unsupported template property"))
        }
    }
}

#[proc_macro_attribute]
pub fn template(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut attrs = TemplateAttributes::default();
    let template_parser = syn::meta::parser(|meta| attrs.parse(meta));
    parse_macro_input!(args with template_parser);
    let input = parse_macro_input!(input as DeriveInput);

    let path = attrs.path.unwrap().value();
    let path = env::current_dir().unwrap().join("templates").join(path);
    let source = fs::read_to_string(&path).unwrap();
    let source = rewrite_source(&path.file_stem().unwrap().to_str().unwrap(), source);

    quote! {
        #[derive(::askama::Template)]
        #[template(source = #source, ext = "html")]
        #input
    }
    .into()
}

fn rewrite_source(name: &str, source: String) -> String {
    let re = Regex::new(COMPONENT_RE).unwrap();
    let import = add_import(re.captures_iter(&source));
    let source = re.replace_all(&source, rewrite_component).into_owned();

    format!(
        "\
        {import}\n\
        {{% macro {name}() %}}\n\
        {source}\n\
        {{% endmacro %}}\n\
        {{% call {name}() %}}",
    )
}

fn add_import(caps: regex::CaptureMatches) -> String {
    let mut import = HashSet::new();
    let mut output = String::new();

    for cap in caps {
        let name = cap.get(1).unwrap().as_str().to_ascii_lowercase();
        import.insert(name);
    }

    for name in import {
        let line = format!("{{%- import \"{name}.html\" as {name}_scope -%}}\n");
        output.push_str(&line);
    }

    output
}

fn rewrite_component(caps: &regex::Captures) -> String {
    let name = caps.get(1).unwrap().as_str().to_ascii_lowercase();
    let args = caps
        .get(2)
        .unwrap()
        .as_str()
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .join(", ");

    format!("{{% call {name}_scope::{name}({args}) %}}")
}

#[test]
fn test_rewrite_source() {
    assert_eq!(
        rewrite_source("index", "<Hello name />".to_string()),
        "\
        {%- import \"hello.html\" as hello_scope -%}\n\n\
        {% macro index() %}\n\
        {% call hello_scope::hello(name) %}\n\
        {% endmacro %}\n\
        {% call index() %}"
    );
}
