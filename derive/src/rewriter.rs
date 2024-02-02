use crate::generator::Buffer;
use crate::node::JsxBlock;
use crate::node::JsxClose;
use crate::node::MacroDef;
use crate::node::Node;
use crate::node::Parsed;
use crate::CompileError;
use std::collections::HashSet;
use std::path::Path;

pub(crate) fn transform_path<P>(path: P) -> String
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

    let parsed = Parsed::new(source)?;
    let source = Rewriter::new(parsed.nodes()).build(&macro_name)?;

    Ok(source)
}

struct Rewriter<'a> {
    nodes: &'a [Node<'a>],
}

impl<'a> Rewriter<'a> {
    fn new(nodes: &'a [Node<'a>]) -> Self {
        Self { nodes }
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
                .nodes
                .iter()
                .filter_map(|node| match node {
                    Node::JsxBlock(node) => Some(node),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        )?;

        // Wrap template in a macro definition.
        self.write_macro(
            buf,
            macro_name,
            self.nodes.iter().find_map(|node| match node {
                Node::MacroDef(node) => Some(node),
                _ => None,
            }),
        )?;

        self.visit_nodes(buf, self.nodes)?;

        self.write_macro_end(buf, macro_name)?;

        Ok(())
    }

    fn visit_nodes(&self, buf: &mut Buffer, nodes: &[Node<'a>]) -> Result<(), CompileError> {
        for node in nodes {
            match node {
                Node::JsxBlock(node) => {
                    self.write_call(buf, node)?;
                }
                Node::JsxClose(node) => {
                    self.write_call_end(buf, node)?;
                }
                Node::Lit(source) => {
                    buf.write(source.val);
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn write_imports(&self, buf: &mut Buffer, tags: &[&JsxBlock<'a>]) -> Result<(), CompileError> {
        let mut imports = HashSet::new();

        for tag in tags {
            let macro_name = normalize(tag.name);
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
        def: Option<&MacroDef<'a>>,
    ) -> Result<(), CompileError> {
        let macro_args = def.map(|m| m.args.join(", ")).unwrap_or_default();

        buf.writeln(&format!("{{% macro {macro_name}({macro_args}) %}}"))
    }

    fn write_macro_end(&self, buf: &mut Buffer, macro_name: &str) -> Result<(), CompileError> {
        buf.writeln(&format!("{{% endmacro {macro_name} %}}"))
    }

    fn write_call(&self, buf: &mut Buffer, tag: &JsxBlock<'a>) -> Result<(), CompileError> {
        let macro_name = normalize(tag.name);
        let macro_args = tag.args.join(", ");

        buf.write(&format!(
            "{{% call {macro_name}_scope::{macro_name}({macro_args}) %}}"
        ));

        if tag.self_closing {
            self.write_call_end(buf, &JsxClose { name: tag.name })?;
        }

        Ok(())
    }

    fn write_call_end(&self, buf: &mut Buffer, _tag: &JsxClose<'a>) -> Result<(), CompileError> {
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
fn test_transform_path() {
    assert_eq!(
        transform_path("templates/hello_world.html"),
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
