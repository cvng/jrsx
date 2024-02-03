#![deny(elided_lifetimes_in_paths)]
#![deny(unreachable_pub)]

mod parser;
mod rewriter;

use parser::Ast;
use rewriter::normalize;
use rewriter::Rewriter;
use std::path::Path;

pub fn transform_path<P: AsRef<Path>>(path: P) -> String {
    let macro_name = normalize(&path);
    let macro_path = path.as_ref().display();

    format!(
        "\
        {{%- import \"{macro_path}\" as {macro_name}_scope -%}}\n\
        {{% call {macro_name}_scope::{macro_name}() %}}{{% endcall %}}\n"
    )
}

pub fn rewrite_source<P: AsRef<Path>>(path: P, source: String) -> String {
    let macro_name = normalize(path);

    let parsed = match Ast::from_str(&source) {
        Ok(parsed) => parsed,
        Err(_) => return source,
    };

    Rewriter::new(&parsed.nodes)
        .build(&macro_name)
        .unwrap_or(source)
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
        rewrite_source("index", "<Hello name />".into()),
        "\
        {%- import \"hello.html\" as hello_scope -%}\n\
        {% macro index() %}\n\
        {% call hello_scope::hello(name) %}{% endcall %}{% endmacro index %}\n"
    );
}
