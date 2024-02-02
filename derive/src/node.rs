#![allow(dead_code)]

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_till;
use nom::character::complete::alpha1;
use nom::character::complete::anychar;
use nom::character::complete::space1;
use nom::combinator::complete;
use nom::combinator::cut;
use nom::combinator::eof;
use nom::combinator::map;
use nom::combinator::not;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::error::ErrorKind;
use nom::error_position;
use nom::multi::many0;
use nom::sequence::delimited;
use nom::sequence::terminated;
use nom::sequence::tuple;
use parser::ParseError;

const JSX_BLOCK_START: &str = "<";
const JSX_BLOCK_END: &str = ">";
const JSX_CLOSE_START: &str = "</";
const JSX_CLOSE_END: &str = ">";
const MACRO_DEF_START: &str = "{#def";
const MACRO_DEF_END: &str = "#}";

type ParseResult<'a, T = &'a str> = Result<(&'a str, T), nom::Err<nom::error::Error<&'a str>>>;

pub(crate) struct Parsed {
    pub(crate) ast: Ast<'static>,
    #[allow(dead_code)]
    pub(crate) source: String,
}

impl Parsed {
    pub(crate) fn new(source: String) -> Result<Self, ParseError> {
        let src = unsafe { std::mem::transmute::<&str, &'static str>(source.as_str()) };
        let ast = Ast::from_str(src)?;

        Ok(Self { ast, source })
    }

    pub(crate) fn nodes(&self) -> &[Node<'_>] {
        &self.ast.nodes
    }
}

#[derive(Debug)]
pub(crate) struct Ast<'a> {
    nodes: Vec<Node<'a>>,
}

impl<'a> Ast<'a> {
    fn from_str(src: &'a str) -> Result<Self, ParseError> {
        let parse = |i: &'a str| Node::many(i);

        match terminated(parse, cut(eof))(src) {
            Ok(("", nodes)) => Ok(Self { nodes }),
            err => panic!("{:#?}", err),
        }
    }
}

struct State;

impl State {
    fn tag_jsx_block_start<'i>(i: &'i str) -> ParseResult<'i> {
        tag(JSX_BLOCK_START)(i)
    }

    fn tag_jsx_block_end<'i>(i: &'i str) -> ParseResult<'i> {
        tag(JSX_BLOCK_END)(i)
    }

    fn tag_jsx_close_start<'i>(i: &'i str) -> ParseResult<'i> {
        tag(JSX_CLOSE_START)(i)
    }

    fn tag_jsx_close_end<'i>(i: &'i str) -> ParseResult<'i> {
        tag(JSX_CLOSE_END)(i)
    }

    fn tag_macro_def_start<'i>(i: &'i str) -> ParseResult<'i> {
        tag(MACRO_DEF_START)(i)
    }

    fn tag_macro_def_end<'i>(i: &'i str) -> ParseResult<'i> {
        tag(MACRO_DEF_END)(i)
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum Node<'a> {
    Lit(Lit<'a>),
    JsxBlock(JsxBlock<'a>),
    JsxClose(JsxClose<'a>),
    MacroDef(MacroDef<'a>),
}

impl<'a> Node<'a> {
    fn many(i: &'a str) -> ParseResult<'a, Vec<Self>> {
        complete(many0(alt((
            map(Lit::parse, Self::Lit),
            map(MacroDef::parse, Self::MacroDef),
            Self::parse,
        ))))(i)
    }

    fn parse(i: &'a str) -> ParseResult<'a, Self> {
        let mut p = delimited(
            |i| Ok((i, "")), // |i| State::tag_jsx_block_start(i),
            alt((
                map(JsxBlock::parse, Self::JsxBlock),
                map(JsxClose::parse, Self::JsxClose),
            )),
            |i| Ok((i, "")), // cut(|i| State::tag_jsx_block_end(i)),
        );

        let result = p(i);

        result
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Lit<'a> {
    pub(crate) val: &'a str,
}

impl<'a> Lit<'a> {
    fn parse(i: &'a str) -> ParseResult<'a, Self> {
        let p_start = alt((
            State::tag_jsx_block_start,
            State::tag_jsx_close_start,
            State::tag_macro_def_start,
        ));

        let (i, _) = not(eof)(i)?;
        let (i, content) = opt(recognize(skip_till(p_start)))(i)?;

        match content {
            Some("") => Err(nom::Err::Error(error_position!(i, ErrorKind::TakeUntil))),
            Some(content) => Ok((i, Self { val: content })),
            None => Ok(("", Self { val: i })),
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct JsxBlock<'a> {
    pub(crate) name: &'a str,
    pub(crate) args: Vec<&'a str>,
    pub(crate) self_closing: bool,
}

impl<'a> JsxBlock<'a> {
    fn parse(i: &'a str) -> ParseResult<'a, Self> {
        let mut p = tuple((
            tag(JSX_BLOCK_START),
            recognize(verify(alpha1, is_uppercase_first)),
            opt(take_till(|c: char| c.to_string() == JSX_BLOCK_END)),
            tag(JSX_BLOCK_END),
        ));

        let (i, (_, name, args, _)) = p(i)?;

        let args = args
            .map(|s| s.trim())
            .unwrap_or("")
            .split_ascii_whitespace()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        let self_closing = args.last().map(|s| s.ends_with('/')).unwrap_or(false);

        let args = args
            .iter()
            .filter(|s| !s.ends_with('/'))
            .copied()
            .collect::<Vec<&str>>();

        Ok((
            i,
            Self {
                name,
                args,
                self_closing,
            },
        ))
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct JsxClose<'a> {
    pub(crate) name: &'a str,
}

impl<'a> JsxClose<'a> {
    fn parse(i: &'a str) -> ParseResult<'a, Self> {
        let mut p = tuple((
            tag(JSX_CLOSE_START),
            recognize(verify(alpha1, is_uppercase_first)),
            tag(JSX_CLOSE_END),
        ));

        let (i, (_, name, _)) = p(i)?;

        Ok((i, Self { name }))
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct MacroDef<'a> {
    pub(crate) args: Vec<&'a str>,
}

impl<'a> MacroDef<'a> {
    fn parse(i: &'a str) -> ParseResult<'a, Self> {
        let mut p = tuple((
            tag(MACRO_DEF_START),
            space1,
            recognize(alpha1),
            space1,
            tag(MACRO_DEF_END),
        ));

        let (i, (_, _, args, _, _)) = p(i)?;

        let args = args.split_ascii_whitespace().collect();

        Ok((i, Self { args }))
    }
}

fn is_uppercase_first(s: &str) -> bool {
    s.chars()
        .next()
        .map(|c| c.is_ascii_uppercase())
        .unwrap_or(false)
}

/// Skips input until `end` was found, but does not consume it.
/// Returns tuple that would be returned when parsing `end`.
fn skip_till<'a, O>(
    end: impl FnMut(&'a str) -> ParseResult<'a, O>,
) -> impl FnMut(&'a str) -> ParseResult<'a, (&'a str, O)> {
    enum Next<O> {
        IsEnd(O),
        NotEnd(char),
    }
    let mut next = alt((map(end, Next::IsEnd), map(anychar, Next::NotEnd)));
    move |start: &'a str| {
        let mut i = start;
        loop {
            let (j, is_end) = next(i)?;
            match is_end {
                Next::IsEnd(lookahead) => return Ok((i, (j, lookahead))),
                Next::NotEnd(_) => i = j,
            }
        }
    }
}

#[test]
fn test_jsx_block() {
    assert_eq!(
        JsxBlock::parse("<Hello name rest=\"rest\" />"),
        Ok((
            "",
            JsxBlock {
                name: "Hello",
                args: vec!["name", "rest=\"rest\""],
                self_closing: true,
            }
        ))
    );

    assert_eq!(
        JsxBlock::parse("<Hello>"),
        Ok((
            "",
            JsxBlock {
                name: "Hello",
                args: vec![],
                self_closing: false,
            }
        ))
    );
}

#[test]
fn test_jsx_close() {
    assert_eq!(
        JsxClose::parse("</Hello>"),
        Ok(("", JsxClose { name: "Hello" }))
    );
}

#[test]
fn test_macro_def() {
    assert_eq!(
        MacroDef::parse("{#def name #}"),
        Ok(("", MacroDef { args: vec!["name"] }))
    );
}

#[test]
fn test_lit() {
    assert_eq!(Lit::parse("Test"), Ok(("", Lit { val: "Test" })));
}

#[test]
fn test_node() {
    assert_eq!(Node::many(""), Ok(("", vec![])));

    assert_eq!(
        Node::many("<Hello />"),
        Ok((
            "",
            vec![Node::JsxBlock(JsxBlock {
                name: "Hello",
                args: vec![],
                self_closing: true,
            })]
        ))
    );

    assert_eq!(
        Node::many("<Hello />\nTest"),
        Ok((
            "",
            vec![
                Node::JsxBlock(JsxBlock {
                    name: "Hello",
                    args: vec![],
                    self_closing: true,
                }),
                Node::Lit(Lit { val: "\nTest" })
            ]
        ))
    );

    assert_eq!(
        Node::many("Test\n<Hello />"),
        Ok((
            "",
            vec![
                Node::Lit(Lit { val: "Test\n" }),
                Node::JsxBlock(JsxBlock {
                    name: "Hello",
                    args: vec![],
                    self_closing: true,
                })
            ],
        ))
    );

    assert_eq!(
        Node::many("</Hello>"),
        Ok(("", vec![Node::JsxClose(JsxClose { name: "Hello" })]))
    );

    assert_eq!(
        Node::many("</Hello>\nTest"),
        Ok((
            "",
            vec![
                Node::JsxClose(JsxClose { name: "Hello" }),
                Node::Lit(Lit { val: "\nTest" })
            ]
        ))
    );

    assert_eq!(
        Node::many("Test\n</Hello>"),
        Ok((
            "",
            vec![
                Node::Lit(Lit { val: "Test\n" }),
                Node::JsxClose(JsxClose { name: "Hello" })
            ]
        ))
    );

    // assert_eq!(Node::many("<"), Ok(("", vec![Node::Lit(Lit { val: "<" })])));
}
