use proc_macro::TokenStream;
use syn::meta::ParseNestedMeta;
use syn::parse::Result;
use syn::parse_macro_input;
use syn::LitStr;

#[proc_macro_attribute]
pub fn template(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = input.to_string();

    let mut attrs = TemplateAttributes::default();
    let tpl_parser = syn::meta::parser(|meta| attrs.parse(meta));
    parse_macro_input!(args with tpl_parser);

    let path = attrs.path.unwrap().value();

    format!(
        r#"
        #[derive(::askama::Template)]
        #[template(path = "{path}")]
        {input}
        "#
    )
    .parse()
    .unwrap()
}

#[derive(Default)]
struct TemplateAttributes {
    path: Option<LitStr>,
}

impl TemplateAttributes {
    fn parse(&mut self, meta: ParseNestedMeta) -> Result<()> {
        if meta.path.is_ident("path") {
            self.path = meta.value()?.parse()?;
            Ok(())
        } else {
            Err(meta.error("unsupported template property"))
        }
    }
}
