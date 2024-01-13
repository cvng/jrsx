use crate::generator::Buffer;
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

pub(crate) fn rewrite_path<P>(path: P) -> String
where
    P: AsRef<Path>,
{
    let macro_name = normalize(&path);
    let macro_path = path.as_ref().display();

    format!(
        "\
        {{%- import \"{macro_path}\" as {macro_name}_scope -%}}\n\
        {{% call {macro_name}_scope::{macro_name}() %}}\n"
    )
}

pub(crate) fn rewrite_source<P>(path: P, source: String) -> Result<String, CompileError>
where
    P: AsRef<Path>,
{
    let macro_name = normalize(path);

    let parsed = Ast::from_str(&source)?;
    let source = Rewriter::new(parsed).build(&macro_name)?;

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

    fn build(&self, macro_name: &str) -> Result<String, CompileError> {
        let mut buf = Buffer::new(0);

        self.rewrite_template(&mut buf, macro_name)?;

        Ok(buf.buf)
    }

    fn rewrite_template(&self, buf: &mut Buffer, macro_name: &str) -> Result<(), CompileError> {
        // Collect imports at the top level. https://github.com/djc/askama/issues/931
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
        )?;

        self.write_macro_def(
            buf,
            macro_name,
            self.ast.nodes.iter().find_map(|node| match node {
                Node::MacroDef(node) => Some(node),
                _ => None,
            }),
        )?;

        self.visit_nodes(buf, &self.ast.nodes)?;

        self.write_macro_end(buf, macro_name)?;

        Ok(())
    }

    fn write_imports(&self, buf: &mut Buffer, blocks: &[&JsxBlock]) -> Result<(), CompileError> {
        let mut imports = HashSet::new();

        for block in blocks {
            let macro_name = normalize(&block.name);
            let macro_path = format!("{macro_name}.html");

            if imports.insert(macro_name.clone()) {
                buf.writeln(&format!(
                    "{{%- import \"{macro_path}\" as {macro_name}_scope -%}}",
                ))?;
            }
        }

        Ok(())
    }

    fn write_macro_def(
        &self,
        buf: &mut Buffer,
        macro_name: &str,
        macro_args: Option<&MacroDef>,
    ) -> Result<(), CompileError> {
        let macro_args = macro_args.map(|m| m.args.join(", ")).unwrap_or_default();

        buf.writeln(&format!("{{% macro {macro_name}({macro_args}) %}}"))
    }

    fn write_macro_end(&self, buf: &mut Buffer, macro_name: &str) -> Result<(), CompileError> {
        buf.writeln(&format!("{{% endmacro {macro_name} %}}"))
    }

    fn visit_nodes(&self, buf: &mut Buffer, nodes: &[Node]) -> Result<(), CompileError> {
        for node in nodes {
            match node {
                Node::JsxBlock(node) => {
                    self.write_macro_call(buf, node)?;
                }
                Node::Source(source) => {
                    buf.write(source);
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn write_macro_call(&self, buf: &mut Buffer, block: &JsxBlock) -> Result<(), CompileError> {
        let macro_name = normalize(&block.name);
        let macro_args = block.args.join(", ");

        buf.writeln(&format!(
            "{{% call {macro_name}_scope::{macro_name}({macro_args}) %}}"
        ))
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
fn test_rewrite_path() {
    assert_eq!(
        rewrite_path("templates/hello_world.html"),
        "\
        {%- import \"templates/hello_world.html\" as hello_world_scope -%}\n\
        {% call hello_world_scope::hello_world() %}\n"
    );
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
