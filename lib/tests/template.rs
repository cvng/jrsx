use jrsx::Template;

#[derive(Template)]
#[template(
    source = "{% macro test() %}OK!{% call caller() %}{% endcall %}{% endmacro %}{% call test() %}Yes!{% endcall %}",
    ext = "html"
)]
struct Index2 {}

#[test]
fn test_template2() {
    assert_eq!(Index2 {}.to_string(), "OK!Yes!");
}
