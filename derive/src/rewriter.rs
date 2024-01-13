use crate::CompileError;
use once_cell::sync::Lazy;
use parser::ParseError;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

static SYNTAX_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r#"({}|{}|{})"#,
        r#"(?<jsx><([A-Z][a-zA-Z0-9]*)\s*([^>/]*)\s*/*?>)"#, // <Hello name />
        r#"(?<def>\{#def\s+(.+)\s+#\})"#,                    // {#def name #}
        r#"(?<txt>.*[\w+\s+]*)"#,
    ))
    .unwrap()
});

pub(crate) fn rewrite_path<P>(path: P) -> Result<String, CompileError>
where
    P: AsRef<Path>,
{
    let macro_name = normalize(&path);
    let macro_import = path.as_ref().display();

    let source = format!(
        "{{%- import \"{macro_import}\" as scope -%}}\n{{% call scope::{macro_name}() %}}\n"
    );

    Ok(source)
}

pub(crate) fn rewrite_source<P>(path: P, source: String) -> Result<String, CompileError>
where
    P: AsRef<Path>,
{
    let macro_name = normalize(path);

    let ast = Ast::from_str(&source)?;
    let rew = Rewriter::new(ast);

    let source = rew.rewrite(&macro_name, source.capacity())?;

    Ok(source)
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

#[derive(Debug, PartialEq)]
enum Node {
    JsxBlock(JsxBlock),
    MacroDef(MacroDef),
    Source(String),
}

#[derive(Debug)]
struct Ast {
    nodes: Vec<Node>,
}

impl Ast {
    fn from_str(src: &str) -> Result<Self, ParseError> {
        let mut nodes = Vec::new();

        for caps in SYNTAX_RE.captures_iter(src) {
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
                caps if caps.name("txt").is_some() => {
                    nodes.push(Node::Source(caps[7].to_owned()));
                }
                _ => unreachable!(),
            }
        }

        Ok(Self { nodes })
    }
}

struct Rewriter {
    ast: Ast,
}

impl Rewriter {
    fn new(ast: Ast) -> Self {
        Self { ast }
    }

    fn rewrite(&self, macro_name: &str, size_hint: usize) -> Result<String, CompileError> {
        let mut buf = String::with_capacity(size_hint);

        self.visit_ast(&mut buf, macro_name);

        Ok(buf)
    }

    fn visit_ast(&self, buf: &mut String, macro_name: &str) {
        self.write_imports(
            buf,
            &self
                .ast
                .nodes
                .iter()
                .filter_map(|node| match node {
                    Node::JsxBlock(node) => Some(node),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        );

        self.write_macro_def(
            buf,
            macro_name,
            self.ast.nodes.iter().find_map(|node| match node {
                Node::MacroDef(node) => Some(node),
                _ => None,
            }),
        );

        self.visit_nodes(buf, &self.ast.nodes);

        self.write_macro_end(buf, macro_name);
    }

    fn write_imports(&self, buf: &mut String, blocks: &[&JsxBlock]) {
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

    fn write_macro_def(&self, buf: &mut String, macro_name: &str, macro_def: Option<&MacroDef>) {
        let macro_args = macro_def.map(|m| m.args.join(", ")).unwrap_or_default();

        buf.push_str(&format!("{{% macro {macro_name}({macro_args}) %}}\n"));
    }

    fn write_macro_end(&self, buf: &mut String, macro_name: &str) {
        buf.push_str(&format!("{{% endmacro {macro_name} %}}\n"));
    }

    fn visit_nodes(&self, buf: &mut String, nodes: &[Node]) {
        for node in nodes {
            match node {
                Node::JsxBlock(node) => {
                    self.write_macro_call(buf, node);
                }
                Node::Source(source) => {
                    buf.push_str(source);
                }
                _ => {}
            }
        }
    }

    fn write_macro_call(&self, buf: &mut String, block: &JsxBlock) {
        let macro_name = normalize(&block.name);
        let macro_args = block.args.join(", ");

        buf.push_str(&format!(
            "{{% call {macro_name}_scope::{macro_name}({macro_args}) %}}\n"
        ));
    }
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
        rewrite_source("index", "<Hello name />".into()).unwrap(),
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
    let parsed = |s| Ast::from_str(s).unwrap();

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
