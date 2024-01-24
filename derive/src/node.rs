use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_till;
use nom::character::complete::alpha1;
use nom::character::complete::anychar;
use nom::character::complete::char;
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
use nom::sequence::terminated;
use nom::sequence::tuple;
use parser::ParseError;

const HTML_TAG_START: &str = "<";
const MACRO_ARGS_START: &str = "{#def";

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

#[derive(Debug, PartialEq)]
pub(crate) enum Node<'a> {
    Lit(Lit<'a>),
    JsxStart(JsxStart<'a>),
    JsxClose(JsxClose<'a>),
    MacroArgs(MacroArgs<'a>),
}

impl<'a> Node<'a> {
    fn many(i: &'a str) -> ParseResult<'a, Vec<Self>> {
        complete(many0(alt((map(Lit::parse, Self::Lit), Self::parse))))(i)
    }

    fn parse(i: &'a str) -> ParseResult<'a, Self> {
        let mut p = alt((
            map(JsxStart::parse, Self::JsxStart),
            map(JsxClose::parse, Self::JsxClose),
            map(MacroArgs::parse, Self::MacroArgs),
        ));

        let result = p(i)?;

        Ok(dbg!(result))
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Lit<'a> {
    pub(crate) val: &'a str,
}

impl<'a> Lit<'a> {
    fn parse(i: &'a str) -> ParseResult<'a, Self> {
        let p_start = alt((tag(HTML_TAG_START), tag(MACRO_ARGS_START)));

        let (i, _) = not(eof)(i)?;
        let (i, content) = opt(recognize(skip_till(p_start)))(i)?;
        let (i, content) = match content {
            Some("") => return Err(nom::Err::Error(error_position!(i, ErrorKind::TakeUntil))),
            Some(content) => (i, content),
            None => ("", i),
        };

        Ok((i, Self { val: content }))
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct JsxStart<'a> {
    pub(crate) name: &'a str,
    pub(crate) args: Vec<&'a str>,
    pub(crate) self_closing: bool,
}

impl<'a> JsxStart<'a> {
    fn parse(i: &'a str) -> ParseResult<'a, Self> {
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
            tag("</"),
            recognize(verify(alpha1, is_uppercase_first)),
            char('>'),
        ));

        let (i, (_, name, _)) = p(i)?;

        Ok((i, Self { name }))
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct MacroArgs<'a> {
    pub(crate) args: Vec<&'a str>,
}

impl<'a> MacroArgs<'a> {
    fn parse(i: &'a str) -> ParseResult<'a, Self> {
        let mut p = tuple((tag("{#def"), space1, recognize(alpha1), space1, tag("#}")));

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
fn test_jsx_start() {
    assert_eq!(
        JsxStart::parse("<Hello name rest=\"rest\" />"),
        Ok((
            "",
            JsxStart {
                name: "Hello",
                args: vec!["name", "rest=\"rest\""],
                self_closing: true,
            }
        ))
    );

    assert_eq!(
        JsxStart::parse("<Hello>"),
        Ok((
            "",
            JsxStart {
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
fn test_macro_args() {
    assert_eq!(
        MacroArgs::parse("{#def name #}"),
        Ok(("", MacroArgs { args: vec!["name"] }))
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
            vec![Node::JsxStart(JsxStart {
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
                Node::JsxStart(JsxStart {
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
                Node::JsxStart(JsxStart {
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

    /*
    assert_eq!(
        Node::many("<"),
        Ok(("", vec![Node::Lit(Lit { val: "<".into() })]))
    );
    */
}
