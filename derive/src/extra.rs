use proc_macro::TokenStream;
use quote::quote;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use syn::meta::ParseNestedMeta;
use syn::parse_macro_input;
use syn::DeriveInput;
use syn::LitStr;

// TODO: https://crates.io/crates/syn-rsx
const COMPONENT_RE: &str = r#"<([A-Z][a-zA-Z0-9]*)\s*([^>/]*)\s*/*?>"#;
const COMPONENT_ARG_RE: &str = r#"\{#def\s+(.+)\s+#\}"#;
const TEMPLATES_DIR: &str = "templates";

#[derive(Default)]
struct TemplateAttributes {
    path: Option<LitStr>,
}

impl TemplateAttributes {
    fn parse(&mut self, meta: ParseNestedMeta<'_>) -> syn::Result<()> {
        if meta.path.is_ident("path") {
            self.path = meta.value()?.parse()?;
            Ok(())
        } else {
            Err(meta.error("unsupported template property"))
        }
    }
}

pub(crate) fn rewrite_template(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut attrs = TemplateAttributes::default();
    let template_parser = syn::meta::parser(|meta| attrs.parse(meta));
    parse_macro_input!(args with template_parser);
    let input = parse_macro_input!(input as DeriveInput);

    let path = attrs.path.unwrap().value();
    let name = input.ident.to_string().to_ascii_lowercase();
    let source = format!(
        "\
        {{%- import \"{path}\" as scope -%}}\n\
        {{% call scope::{name}() %}}"
    );

    copy_templates(); //

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
    let name = name.replace('.', "_");
    let mut args = String::new();
    let re2 = Regex::new(COMPONENT_ARG_RE).unwrap();
    if let Some(caps) = re2.captures(&source) {
        args = caps.get(1).unwrap().as_str().to_string();
    }
    let source = re2.replace_all(&source, "");

    format!(
        "\
        {import}\
        {{% macro {name}({args}) %}}\n\
        {source}\
        {{% endmacro %}}\n",
    )
}

fn add_import(caps: regex::CaptureMatches<'_, '_>) -> String {
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

fn rewrite_component(caps: &regex::Captures<'_>) -> String {
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
        {%- import \"hello.html\" as hello_scope -%}\n\
        {% macro index() %}\n\
        {% call hello_scope::hello(name) %}\
        {% endmacro %}\n"
    );
}

fn copy_templates() {
    for path in fs::read_dir(format!("{}/in", TEMPLATES_DIR))
        .unwrap()
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    {
        let name = path.file_stem().unwrap().to_str().unwrap();
        let source = fs::read_to_string(&path).unwrap();
        let source = rewrite_source(name, source);

        fs::write(
            format!(
                "{}/out/{}",
                TEMPLATES_DIR,
                path.file_name().unwrap().to_str().unwrap()
            ),
            source,
        )
        .unwrap();
    }
}