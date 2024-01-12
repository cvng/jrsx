use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

// TODO: https://crates.io/crates/syn-rsx
const COMPONENT_RE: &str = r#"<([A-Z][a-zA-Z0-9]*)\s*([^>/]*)\s*/*?>"#;
const COMPONENT_ARG_RE: &str = r#"\{#def\s+(.+)\s+#\}"#;

pub(crate) fn rewrite_path(path: String) -> String {
    format!(
        "\
        {{%- import \"{}\" as scope -%}}\n\
        {{% call scope::{}() %}}",
        path,
        as_identifier(Path::new(&path))
    )
}

pub(crate) fn rewrite_source(path: &Path, source: String) -> String {
    let re = Regex::new(COMPONENT_RE).unwrap();
    let import = add_import(re.captures_iter(&source));
    let source = re.replace_all(&source, rewrite_component).into_owned();
    let name = as_identifier(path);
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

fn as_identifier(path: &Path) -> String {
    path.file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_ascii_lowercase()
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
        rewrite_source(Path::new("index"), "<Hello name />".to_string()),
        "\
        {%- import \"hello.html\" as hello_scope -%}\n\
        {% macro index() %}\n\
        {% call hello_scope::hello(name) %}\
        {% endmacro %}\n"
    );
}
