#![allow(unused)]

use crate::generator::Buffer;
use crate::CompileError;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_till;
use nom::bytes::complete::take_while;
use nom::character::complete::alpha1;
use nom::character::complete::char;
use nom::character::complete::space1;
use nom::character::is_alphabetic;
use nom::combinator::cond;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::sequence::tuple;
use once_cell::sync::Lazy;
use parser::ParseError;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

type ParseResult<'a, T = &'a str> = nom::IResult<&'a str, T>;

static SYNTAX_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r#"({}|{}|{}|{})"#,
        r#"(?<jsx_start><([A-Z][a-zA-Z0-9]*)\s*([^>/]*)\s*/*?>)"#, // <Hello name />
        r#"(?<jsx_end></([A-Z][a-zA-Z0-9]*)\s*>)"#,                // </Hello>
        r#"(?<macro_args>\{#def\s+(.+)\s+#\})"#,                   // {#def name #}
        r#"(?<source>.*[\w+\s+]*)"#,
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
        {{% call {macro_name}_scope::{macro_name}() %}}{{% endcall %}}\n"
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
struct JsxStart {
    name: String,
    args: Vec<String>,
    self_closing: bool,
}

#[derive(Debug, PartialEq)]
struct JsxEnd {
    name: String,
}

#[derive(Debug, PartialEq)]
struct MacroArgs {
    args: Vec<String>,
}

#[derive(Debug, PartialEq)]
struct Source {
    text: String,
}

#[derive(Debug, PartialEq)]
enum Node {
    JsxStart(JsxStart),
    JsxEnd(JsxEnd),
    MacroArgs(MacroArgs),
    Source(Source),
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
                caps if caps.name("jsx_start").is_some() => {
                    nodes.push(Node::JsxStart(JsxStart {
                        name: caps[3].to_owned(),
                        args: caps[4].split_whitespace().map(|s| s.to_owned()).collect(),
                        self_closing: caps[2].ends_with("/>"),
                    }));
                }
                caps if caps.name("jsx_end").is_some() => {
                    nodes.push(Node::JsxEnd(JsxEnd {
                        name: caps[6].to_owned(),
                    }));
                }
                caps if caps.name("macro_args").is_some() => {
                    nodes.push(Node::MacroArgs(MacroArgs {
                        args: caps[8].split_whitespace().map(|s| s.to_owned()).collect(),
                    }));
                }
                caps if caps.name("source").is_some() => {
                    nodes.push(Node::Source(Source {
                        text: caps[9].to_owned(),
                    }));
                }
                _ => unreachable!(),
            }
        }

        Ok(Self { nodes })
    }

    fn jsx_start(i: &str) -> ParseResult<'_, JsxStart> {
        let mut p = tuple((
            char('<'),
            recognize(verify(alpha1, is_uppercase_first)),
            opt(space1),
            take_till(|c: char| c.eq(&'/') || c.eq(&'>')),
            opt(char('/')),
            char('>'),
        ));

        let (i, (_, name, _, args, self_closing, _)) = p(i)?;

        Ok((
            i,
            JsxStart {
                name: name.to_owned(),
                args: args.split_whitespace().map(|s| s.to_owned()).collect(),
                self_closing: self_closing.is_some(),
            },
        ))
    }
}

fn is_uppercase_first(s: &str) -> bool {
    s.chars().next().map(|s| s.is_uppercase()).unwrap_or(false)
}

#[test]
fn test_jsx_start() {
    assert_eq!(
        Ast::jsx_start("<Hello name rest=\"rest\" />"),
        Ok((
            "",
            JsxStart {
                name: "Hello".into(),
                args: vec!["name".into(), "rest=\"rest\"".into()],
                self_closing: true,
            }
        ))
    );

    assert_eq!(
        Ast::jsx_start("<Hello>"),
        Ok((
            "",
            JsxStart {
                name: "Hello".into(),
                args: vec![],
                self_closing: false,
            }
        ))
    );
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
                    Node::JsxStart(node) => Some(node),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        )?;

        // Wrap template in a macro definition.
        self.write_macro(
            buf,
            macro_name,
            self.ast.nodes.iter().find_map(|node| match node {
                Node::MacroArgs(node) => Some(node),
                _ => None,
            }),
        )?;

        self.visit_nodes(buf, &self.ast.nodes)?;

        self.write_macro_end(buf, macro_name)?;

        Ok(())
    }

    fn visit_nodes(&self, buf: &mut Buffer, nodes: &[Node]) -> Result<(), CompileError> {
        for node in nodes {
            match node {
                Node::JsxStart(node) => {
                    self.write_call(buf, node)?;
                }
                Node::JsxEnd(node) => {
                    self.write_call_end(buf, node)?;
                }
                Node::Source(source) => {
                    buf.write(&source.text);
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn write_imports(&self, buf: &mut Buffer, tags: &[&JsxStart]) -> Result<(), CompileError> {
        let mut imports = HashSet::new();

        for tag in tags {
            let macro_name = normalize(&tag.name);
            let macro_path = format!("{macro_name}.html");

            if imports.insert(macro_name.clone()) {
                buf.writeln(&format!(
                    "{{%- import \"{macro_path}\" as {macro_name}_scope -%}}",
                ))?;
            }
        }

        Ok(())
    }

    fn write_macro(
        &self,
        buf: &mut Buffer,
        macro_name: &str,
        macro_args: Option<&MacroArgs>,
    ) -> Result<(), CompileError> {
        let macro_args = macro_args.map(|m| m.args.join(", ")).unwrap_or_default();

        buf.writeln(&format!("{{% macro {macro_name}({macro_args}) %}}"))
    }

    fn write_macro_end(&self, buf: &mut Buffer, macro_name: &str) -> Result<(), CompileError> {
        buf.writeln(&format!("{{% endmacro {macro_name} %}}"))
    }

    fn write_call(&self, buf: &mut Buffer, tag: &JsxStart) -> Result<(), CompileError> {
        let macro_name = normalize(&tag.name);
        let macro_args = tag.args.join(", ");

        buf.write(&format!(
            "{{% call {macro_name}_scope::{macro_name}({macro_args}) %}}"
        ));

        if tag.self_closing {
            self.write_call_end(
                buf,
                &JsxEnd {
                    name: tag.name.clone(),
                },
            )?;
        }

        Ok(())
    }

    fn write_call_end(&self, buf: &mut Buffer, _tag: &JsxEnd) -> Result<(), CompileError> {
        buf.write("{% endcall %}");
        Ok(())
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
        .to_lowercase()
        .replace(['-', '.'], "_")
}

#[test]
fn test_rewrite_path() {
    assert_eq!(
        rewrite_path("templates/hello_world.html"),
        "\
        {%- import \"templates/hello_world.html\" as hello_world_scope -%}\n\
        {% call hello_world_scope::hello_world() %}{% endcall %}\n"
    );
}

#[test]
fn test_rewrite_source() {
    assert_eq!(
        rewrite_source("index", "<Hello name />".into()).unwrap(),
        "\
        {%- import \"hello.html\" as hello_scope -%}\n\
        {% macro index() %}\n\
        {% call hello_scope::hello(name) %}{% endcall %}{% endmacro index %}\n"
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
        Some(&Node::JsxStart(JsxStart {
            name: "Hello".into(),
            args: vec!["name".into()],
            self_closing: true,
        }))
    );

    assert_eq!(
        parsed("Test\n<Hello name />").nodes.first(),
        Some(&Node::Source(Source {
            text: "Test\n".into()
        }))
    );

    assert_eq!(
        parsed("<Hello name />\nTest").nodes.last(),
        Some(&Node::Source(Source {
            text: "\nTest".into()
        }))
    );

    assert_eq!(
        parsed("</Hello>").nodes.first(),
        Some(&Node::JsxEnd(JsxEnd {
            name: "Hello".into()
        }))
    );

    assert_eq!(
        parsed("Test\n</Hello>").nodes.last(),
        Some(&Node::JsxEnd(JsxEnd {
            name: "Hello".into()
        }))
    );
}
