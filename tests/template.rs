use jrsx::make_build_templates;
use jrsx::template;

make_build_templates!();

#[template(path = "index.html")]
struct Index<'a> {
    name: &'a str,
}

#[test]
fn test_template() {
    build_templates();

    assert_eq!(
        Index { name: "world" }.to_string(),
        "\n\n\
        <h1>Hello, world!</h1>\n\n\n\
        <h1>Hello, world!</h1>\n\n\n\
        <h1>Hello, world!</h1>\n\n"
    );
}
