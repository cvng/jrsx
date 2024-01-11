use jrsx::template;

#[derive(askama::Template)]
#[template(path = "index.dist.html")]
struct Index<'a> {
    name: &'a str,
}

#[test]
fn test_template() {
    assert_eq!(
        Index { name: "world" }.to_string(),
        "\n\n\n\
        <h1>Hello, world!</h1>\n\n\n\
        <h1>Hello, world!</h1>\n\n\n\
        <h1>Hello, world!</h1>\n\n"
    );
}
