use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn template(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr = attr.to_string();
    let item = item.to_string();

    format!(
        r#"
        #[derive(::askama::Template)]
        #[template({attr})]
        {item}
        "#
    )
    .parse()
    .unwrap()
}
