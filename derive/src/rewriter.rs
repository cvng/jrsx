use crate::generator::Buffer;
use crate::node::Ast;
use crate::node::JsxEnd;
use crate::node::JsxStart;
use crate::node::MacroArgs;
use crate::node::Node;
use crate::CompileError;
use std::collections::HashSet;
use std::path::Path;

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
