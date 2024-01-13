use jrsx::Template;

#[derive(Template)]
#[template(path = "index.html")]
struct Index<'a> {
    name: &'a str,
}

#[test]
fn test_template() {
    assert_eq!(
        Index { name: "world" }.to_string(),
        "\n\n\n\n\
        <h1>Hello, world!</h1>\n\n\n\n\
        <h1>Hello, world!</h1>\n\n\n\n\
        <h1>Hello, world!</h1>\n"
    );
}

#[derive(Template)]
#[template(
    source = "\
{% macro index() %}{% endmacro %}{% call index() %}{# endcall #}",
    ext = "html"
)]
struct Index2 {}

#[test]
fn test_template2() {
    assert_eq!(
        Index2 {}.to_string(),
        "", // TODO: "\n\n\n\n<div>{# caller() #}</div>\n"
    );
}
