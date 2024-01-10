use jrsx::template;

#[template(path = "index.html")]
struct Index<'a> {
    name: &'a str,
}

#[test]
fn test_template() {
    assert_eq!(
        Index { name: "world" }.to_string().trim(),
        "<h1>Hello, world!</h1>"
    );
}
