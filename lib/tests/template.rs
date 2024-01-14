use jrsx::Template;

#[derive(Template)]
#[template(
    source = "{% extends \"index2.html\" %}{% block content %}{% call super() %}{% endcall %}{% endblock %}",
    ext = "html"
)]
struct Index2 {}

#[test]
fn test_template2() {
    assert_eq!(Index2 {}.to_string(), "Super!");
}
