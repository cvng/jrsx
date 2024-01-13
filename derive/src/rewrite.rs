use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

static JSX_BLOCK_RE: Lazy<Regex> = Lazy::new(|| {
    let re = Regex::new(r#"<([A-Z][a-zA-Z0-9]*)\s*([^>/]*)\s*/*?>"#); // <Hello name />
    re.unwrap()
});

static MACRO_DEF_RE: Lazy<Regex> = Lazy::new(|| {
    let re = Regex::new(r#"\{#def\s+(.+)\s+#\}"#); // {#def name #}
    re.unwrap()
});

static SYNTAX_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        "({}|{}|{})",
        format!(r#"(?<jsx>{})"#, JSX_BLOCK_RE.as_str()),
        format!(r#"(?<def>{})"#, MACRO_DEF_RE.as_str()),
        format!(r#"(?<src>{})"#, r#"[\w+\s+]*"#),
    ))
    .unwrap()
});

#[derive(Debug, PartialEq)]
struct Ast {
    nodes: Vec<Node>,
}

#[derive(Debug, PartialEq)]
enum Node {
    JsxBlock(JsxBlock),
    MacroDef(MacroDef),
    Source(String),
}

#[derive(Debug, PartialEq)]
struct JsxBlock {
    name: String,
    args: Vec<String>,
}

#[derive(Debug, PartialEq)]
struct MacroDef {
    args: Vec<String>,
}

fn parsed(source: &str) -> Ast {
    let mut nodes = Vec::new();

    for caps in SYNTAX_RE.captures_iter(source) {
        match caps {
            caps if caps.name("jsx").is_some() => {
                let name = caps[3].to_owned();
                let args = caps[4]
                    .split_ascii_whitespace()
                    .map(|s| s.to_owned())
                    .collect();

                nodes.push(Node::JsxBlock(JsxBlock { name, args }));
            }
            caps if caps.name("def").is_some() => {
                let args = caps[6]
                    .split_ascii_whitespace()
                    .map(|s| s.to_owned())
                    .collect();

                nodes.push(Node::MacroDef(MacroDef { args }));
            }
            caps if caps.name("src").is_some() => {
                nodes.push(Node::Source(caps[7].to_owned()));
            }
            _ => unreachable!(),
        }
    }

    Ast { nodes }
}

pub(crate) fn rewrite_path<P>(path: P) -> String
where
    P: AsRef<Path>,
{
    let macro_name = normalize(&path);
    let macro_import = path.as_ref().display();

    format!("{{%- import \"{macro_import}\" as scope -%}}\n{{% call scope::{macro_name}() %}}\n")
}

pub(crate) fn rewrite_source<P>(path: P, source: String) -> String
where
    P: AsRef<Path>,
{
    let mut buf = String::with_capacity(source.capacity());

    let ast = parsed(&source);
    let macro_name = normalize(path);

    visit(&mut buf, &ast, source, &macro_name);

    buf
}

fn visit(buf: &mut String, ast: &Ast, source: String, macro_name: &str) {
    write_imports(
        buf,
        ast.nodes
            .iter()
            .filter_map(|node| {
                if let Node::JsxBlock(node) = node {
                    Some(node)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .as_slice(),
    );
    write_macro_def(
        buf,
        macro_name,
        ast.nodes.iter().find_map(|node| {
            if let Node::MacroDef(node) = node {
                Some(node)
            } else {
                None
            }
        }),
    );
    write_macro_body(buf, source);
    write_macro_end(buf, macro_name);
}

fn write_imports(buf: &mut String, blocks: &[&JsxBlock]) {
    let mut imports = HashSet::new();

    for block in blocks {
        let macro_name = normalize(&block.name);
        let macro_import = format!("{macro_name}.html");

        if imports.insert(macro_name.clone()) {
            buf.push_str(&format!(
                "{{%- import \"{macro_import}\" as {macro_name}_scope -%}}\n",
            ));
        }
    }
}

fn write_macro_def(buf: &mut String, macro_name: &str, macro_def: Option<&MacroDef>) {
    let macro_args = macro_def.map(|m| m.args.join(", ")).unwrap_or_default();

    buf.push_str(&format!("{{% macro {macro_name}({macro_args}) %}}\n"));
}

fn write_macro_end(buf: &mut String, macro_name: &str) {
    buf.push_str(&format!("{{% endmacro {macro_name} %}}\n"));
}

fn write_macro_body(buf: &mut String, source: String) {
    let source = MACRO_DEF_RE.replace_all(&source, "");
    let source = JSX_BLOCK_RE.replace_all(&source, replace_macro_call);

    buf.push_str(&source);
}

fn replace_macro_call(caps: &regex::Captures<'_>) -> String {
    let macro_name = normalize(&caps[1]);
    let macro_args = caps[2]
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .join(", ");

    format!("{{% call {macro_name}_scope::{macro_name}({macro_args}) %}}\n")
}

fn normalize<P>(path: P) -> String
where
    P: AsRef<Path>,
{
    path.as_ref()
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_ascii_lowercase()
        .replace(['-', '.'], "_")
}

#[test]
fn test_rewrite_source() {
    assert_eq!(
        rewrite_source("index", "<Hello name />".into()),
        "\
        {%- import \"hello.html\" as hello_scope -%}\n\
        {% macro index() %}\n\
        {% call hello_scope::hello(name) %}\n\
        {% endmacro index %}\n"
    );
}

#[test]
fn test_normalize() {
    assert_eq!(normalize("templates/hello_world.html"), "hello_world");
    assert_eq!(normalize("templates/hello-world.html"), "hello_world");
    assert_eq!(normalize("templates/hello.world.html"), "hello_world");
}

#[test]
fn test_parsed() {
    assert_eq!(parsed("<Hello name />").nodes.len(), 1);
    assert_eq!(
        parsed("<Hello name />").nodes.first(),
        Some(&Node::JsxBlock(JsxBlock {
            name: "Hello".into(),
            args: vec!["name".into()],
        }))
    );
    assert_eq!(
        parsed("Test\n<Hello name />").nodes.first(),
        Some(&Node::Source("Test\n".into()))
    );
    assert_eq!(
        parsed("<Hello name />\nTest").nodes.last(),
        Some(&Node::Source("\nTest".into()))
    );
}
