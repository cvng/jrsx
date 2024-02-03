use crate::parser::JsxBlock;
use crate::parser::JsxClose;
use crate::parser::MacroDef;
use crate::parser::Node;
use std::collections::HashSet;
use std::path::Path;

pub(crate) struct CompileError(String);

pub(crate) struct Rewriter<'a> {
    nodes: &'a [Node<'a>],
}

impl<'a> Rewriter<'a> {
    pub(crate) fn new(nodes: &'a [Node<'a>]) -> Self {
        Self { nodes }
    }

    pub(crate) fn build(&self, macro_name: &str) -> Result<String, CompileError> {
        let mut buf = Buffer::new();

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

pub(crate) struct Buffer {
    pub(crate) buf: String,
}

impl Buffer {
    pub(crate) fn new() -> Self {
        Self { buf: String::new() }
    }

    pub(crate) fn writeln(&mut self, s: &str) -> Result<(), CompileError> {
        if !s.is_empty() {
            self.write(s);
        }
        self.buf.push('\n');
        Ok(())
    }

    pub(crate) fn write(&mut self, s: &str) {
        self.buf.push_str(s);
    }
}

pub(crate) fn normalize<P>(path: P) -> String
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
