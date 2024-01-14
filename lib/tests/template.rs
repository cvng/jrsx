use jrsx::Template;

#[derive(Template)]
#[template(source = "{# call caller() #}", ext = "html")]
struct Index2 {}

#[test]
fn test_template2() {
    assert_eq!(Index2 {}.to_string(), "");
}
