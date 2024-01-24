use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_till;
use nom::bytes::complete::take_while;
use nom::character::complete::alpha1;
use nom::character::complete::char;
use nom::character::complete::space1;
use nom::combinator::map;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::sequence::tuple;
use parser::ParseError;

type ParseResult<'a, T = &'a str> = nom::IResult<&'a str, T>;

#[derive(Debug, PartialEq)]
pub(crate) struct JsxStart {
    pub(crate) name: String,
    pub(crate) args: Vec<String>,
    pub(crate) self_closing: bool,
}

#[derive(Debug, PartialEq)]
pub(crate) struct JsxEnd {
    pub(crate) name: String,
}

#[derive(Debug, PartialEq)]
pub(crate) struct MacroArgs {
    pub(crate) args: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Source {
    pub(crate) text: String,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Node {
    JsxStart(JsxStart),
    JsxEnd(JsxEnd),
    MacroArgs(MacroArgs),
    Source(Source),
}

pub(crate) struct Parsed {
    pub(crate) ast: Ast,
    #[allow(dead_code)]
    pub(crate) source: String,
}

impl Parsed {
    pub(crate) fn new(source: String) -> Result<Self, ParseError> {
        let ast = Ast::from_str(source.as_str())?;

        Ok(Self { ast, source })
    }
}

#[derive(Debug)]
pub(crate) struct Ast {
    pub(crate) nodes: Vec<Node>,
}

impl Ast {
    fn from_str(src: &str) -> Result<Self, ParseError> {
        let mut nodes = vec![];

        let mut i = src;

        while !i.is_empty() {
            let (i2, node) = Self::node(i).unwrap(); // TODO: ?

            i = i2;

            nodes.push(node);
        }

        Ok(Self { nodes })
    }

    fn node(i: &str) -> ParseResult<'_, Node> {
        let mut p = alt((
            map(|i| Self::jsx_start(i), Node::JsxStart),
            map(|i| Self::jsx_end(i), Node::JsxEnd),
            map(|i| Self::macro_args(i), Node::MacroArgs),
            map(|i| Self::source(i), Node::Source),
        ));

        let (i, node) = p(i)?;

        Ok(dbg!(i, node))
    }

    fn jsx_start(i: &str) -> ParseResult<'_, JsxStart> {
        let mut p = tuple((
            tag("<"),
            recognize(verify(alpha1, is_uppercase_first)),
            opt(take_till(|c| c == '>')),
            char('>'),
        ));

        let (i, (_, name, args, _)) = p(i)?;

        let args = args
            .map(|s| s.trim())
            .unwrap_or("")
            .split(' ')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_owned())
            .collect::<Vec<_>>();

        let self_closing = args.last().map(|s| s.ends_with('/')).unwrap_or(false);

        let args = args
            .iter()
            .filter(|s| !s.ends_with('/'))
            .map(|s| s.to_owned())
            .collect::<Vec<_>>();

        Ok((
            i,
            JsxStart {
                name: name.to_owned(),
                args,
                self_closing,
            },
        ))
    }

    fn jsx_end(i: &str) -> ParseResult<'_, JsxEnd> {
        let mut p = tuple((
            tag("</"),
            recognize(verify(alpha1, is_uppercase_first)),
            char('>'),
        ));

        let (i, (_, name, _)) = p(i)?;

        Ok((
            i,
            JsxEnd {
                name: name.to_owned(),
            },
        ))
    }

    fn macro_args(i: &str) -> ParseResult<'_, MacroArgs> {
        let mut p = tuple((tag("{#def"), space1, recognize(alpha1), space1, tag("#}")));

        let (i, (_, _, name, _, _)) = p(i)?;

        Ok((
            i,
            MacroArgs {
                args: vec![name.to_owned()],
            },
        ))
    }

    fn source(i: &str) -> ParseResult<'_, Source> {
        let p = take_while(|c| c != '<' && c != '{');

        let (i, text) = p(i)?;

        Ok((
            i,
            Source {
                text: text.to_owned(),
            },
        ))
    }
}

fn is_uppercase_first(s: &str) -> bool {
    s.chars().next().map(|s| s.is_uppercase()).unwrap_or(false)
}

#[test]
fn test_jsx_start() {
    assert_eq!(
        Ast::jsx_start("<Hello name rest=\"rest\" />"),
        Ok((
            "",
            JsxStart {
                name: "Hello".into(),
                args: vec!["name".into(), "rest=\"rest\"".into()],
                self_closing: true,
            }
        ))
    );

    assert_eq!(
        Ast::jsx_start("<Hello>"),
        Ok((
            "",
            JsxStart {
                name: "Hello".into(),
                args: vec![],
                self_closing: false,
            }
        ))
    );
}

#[test]
fn test_jsx_end() {
    assert_eq!(
        Ast::jsx_end("</Hello>"),
        Ok((
            "",
            JsxEnd {
                name: "Hello".into(),
            }
        ))
    );
}

#[test]
fn test_macro_args() {
    assert_eq!(
        Ast::macro_args("{#def name #}"),
        Ok((
            "",
            MacroArgs {
                args: vec!["name".into()],
            }
        ))
    );
}

#[test]
fn test_source() {
    assert_eq!(
        Ast::source("Test"),
        Ok((
            "",
            Source {
                text: "Test".into(),
            }
        ))
    );
}

#[test]
fn test_from_str() {
    assert_eq!(
        Ast::from_str("<Hello />").unwrap().nodes,
        vec![Node::JsxStart(JsxStart {
            name: "Hello".into(),
            args: vec![],
            self_closing: true,
        })]
    );

    assert_eq!(
        Ast::from_str("<Hello />\nTest").unwrap().nodes,
        vec![
            Node::JsxStart(JsxStart {
                name: "Hello".into(),
                args: vec![],
                self_closing: true,
            }),
            Node::Source(Source {
                text: "\nTest".into()
            })
        ]
    );

    assert_eq!(
        Ast::from_str("Test\n<Hello />").unwrap().nodes,
        vec![
            Node::Source(Source {
                text: "Test\n".into()
            }),
            Node::JsxStart(JsxStart {
                name: "Hello".into(),
                args: vec![],
                self_closing: true,
            })
        ],
    );

    assert_eq!(
        Ast::from_str("</Hello>").unwrap().nodes,
        vec![Node::JsxEnd(JsxEnd {
            name: "Hello".into()
        })]
    );

    assert_eq!(
        Ast::from_str("</Hello>\nTest").unwrap().nodes,
        vec![
            Node::JsxEnd(JsxEnd {
                name: "Hello".into()
            }),
            Node::Source(Source {
                text: "\nTest".into()
            })
        ]
    );

    assert_eq!(
        Ast::from_str("Test\n</Hello>").unwrap().nodes,
        vec![
            Node::Source(Source {
                text: "Test\n".into()
            }),
            Node::JsxEnd(JsxEnd {
                name: "Hello".into()
            })
        ]
    );
}
