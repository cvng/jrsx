use proc_macro::TokenStream;
use quote::quote;
use regex::Regex;
use std::env;
use std::fs;
use syn::meta::ParseNestedMeta;
use syn::parse::Result;
use syn::parse_macro_input;
use syn::DeriveInput;
use syn::LitStr;

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
    let source = fs::read_to_string(path).unwrap();
    let source = rewrite_source(&source);

    quote! {
        #[derive(::askama::Template)]
        #[template(source = #source, ext = "html")]
        #input
    }
    .into()
}

const COMPONENT_RE: &str = r#"<([A-Z][a-zA-Z0-9]*)\s*([^>/]*)\s*/*?>"#;

fn rewrite_source(source: &str) -> String {
    let re = Regex::new(COMPONENT_RE).unwrap();
    re.replace_all(source, rewrite_component).into_owned()
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

    format!(
        "\
        {{%- import \"{name}.html\" as {name}_scope -%}}\n\
        {{% call {name}_scope::{name}({args}) %}}",
    )
}

#[test]
fn test_rewrite_source() {
    assert_eq!(
        rewrite_source("<Hello name />"),
        "\
        {%- import \"hello.html\" as hello_scope -%}\n\
        {% call hello_scope::hello(name) %}"
    );
}
