use std::ffi::OsString;

#[derive(Debug, PartialEq)]
enum ComposedArgumentsEvent {
    ArgBlock(Vec<OsString>),
    OpenBracket,
    CloseBracket,
    Ampersand,
    Semicolon,
    Circumflex,
}

#[derive(Debug, PartialEq)]
pub enum ComposedArgument {
    Simple(Vec<OsString>),
    Parallel(Vec<Box<ComposedArgument>>),
    Sequential(Vec<Box<ComposedArgument>>),
    Race(Vec<Box<ComposedArgument>>),
}

// no gen blocks, so vectors everywhere
#[allow(unused_assignments)]
fn parse1(
    a: impl IntoIterator<Item = impl Into<std::ffi::OsString>>,
) -> Vec<ComposedArgumentsEvent> {
    let mut ret = Vec::with_capacity(4);
    let mut buf: Vec<OsString> = vec!["w".into()];
    macro_rules! commit_buf {
        () => {
            if buf.len() > 1 {
                ret.push(ComposedArgumentsEvent::ArgBlock(buf));
            }
            buf = vec!["w".into()];
        };
    }
    for x in a.into_iter() {
        let x: OsString = x.into();
        if x == "&" {
            commit_buf!();
            ret.push(ComposedArgumentsEvent::Ampersand);
        } else if x == ";" {
            commit_buf!();
            ret.push(ComposedArgumentsEvent::Semicolon);
        } else if x == "^" {
            commit_buf!();
            ret.push(ComposedArgumentsEvent::Circumflex);
        } else if x == "(" {
            commit_buf!();
            ret.push(ComposedArgumentsEvent::OpenBracket);
        } else if x == ")" {
            commit_buf!();
            ret.push(ComposedArgumentsEvent::CloseBracket);
        } else {
            buf.push(x);
        }
    }
    commit_buf!();
    ret
}

#[test]
fn test_parse1() {
    assert_eq!(
        parse1(vec!["qqq"]),
        vec![ComposedArgumentsEvent::ArgBlock(vec![
            "w".into(),
            "qqq".into()
        ])]
    );
    assert_eq!(
        parse1(vec!["qqq", "www", "--", "&&"]),
        vec![ComposedArgumentsEvent::ArgBlock(vec![
            "w".into(),
            "qqq".into(),
            "www".into(),
            "--".into(),
            "&&".into()
        ])]
    );
    assert_eq!(
        parse1(vec!["qqq", "&", "eee"]),
        vec![
            ComposedArgumentsEvent::ArgBlock(vec!["w".into(), "qqq".into()]),
            ComposedArgumentsEvent::Ampersand,
            ComposedArgumentsEvent::ArgBlock(vec!["w".into(), "eee".into()])
        ]
    );
    assert_eq!(
        parse1(vec!["qqq", "&", "(", "eee", "fff", ";", "tt", ")"]),
        vec![
            ComposedArgumentsEvent::ArgBlock(vec!["w".into(), "qqq".into()]),
            ComposedArgumentsEvent::Ampersand,
            ComposedArgumentsEvent::OpenBracket,
            ComposedArgumentsEvent::ArgBlock(vec!["w".into(), "eee".into(), "fff".into()]),
            ComposedArgumentsEvent::Semicolon,
            ComposedArgumentsEvent::ArgBlock(vec!["w".into(), "tt".into()]),
            ComposedArgumentsEvent::CloseBracket,
        ]
    );
}

pub fn parse(
    a: impl IntoIterator<Item = impl Into<std::ffi::OsString>>,
) -> anyhow::Result<ComposedArgument> {
    let mut events = parse1(a);
    let mut current_event = 0;

    enum Thing {
        /// Index into `events`
        Terminal(usize),
        Composed(ComposedArgument),
        BracketedExpression(ComposedArgument),
    }

    #[derive(Debug)]
    enum ThingType {
        // terminals:
        Empty,
        ArgBlock,
        OpenBracket,
        CloseBracket,
        Ampersand,
        Semicolon,
        Circumflex,
        // non-terminals:
        Simple,
        Parallel,
        Sequential,
        Race,
        Bracketed,
    }
    use ThingType::*;

    let mut expr: Vec<Thing> = vec![];

    let thing_type = |xx: Option<&Thing>, evts: &[ComposedArgumentsEvent]| -> ThingType {
        if let Some(x) = xx {
            match x {
                Thing::Terminal(t) => match &evts[*t] {
                    ComposedArgumentsEvent::ArgBlock(..) => ArgBlock,
                    ComposedArgumentsEvent::OpenBracket => OpenBracket,
                    ComposedArgumentsEvent::CloseBracket => CloseBracket,
                    ComposedArgumentsEvent::Ampersand => Ampersand,
                    ComposedArgumentsEvent::Semicolon => Semicolon,
                    ComposedArgumentsEvent::Circumflex => Circumflex,
                },
                Thing::Composed(composed_argument) => match composed_argument {
                    ComposedArgument::Simple(..) => Simple,
                    ComposedArgument::Parallel(..) => Parallel,
                    ComposedArgument::Sequential(..) => Sequential,
                    ComposedArgument::Race(..) => Race,
                },
                Thing::BracketedExpression(..) => Bracketed,
            }
        } else {
            Empty
        }
    };

    #[allow(unused_variables)]
    loop {
        let more_events = events.len() > current_event;
        let cur_typ = thing_type(expr.last(), &events);
        let prev_typ = thing_type(expr.iter().nth_back(1), &events);
        let penul_typ = thing_type(expr.iter().nth_back(2), &events);

        match (penul_typ, prev_typ, cur_typ, more_events) {
            (_, _, ArgBlock, _) => {
                let Some(Thing::Terminal(x)) = expr.pop() else {
                    unreachable!()
                };
                let ComposedArgumentsEvent::ArgBlock(ref mut y) = events[x] else {
                    unreachable!()
                };
                expr.push(Thing::Composed(ComposedArgument::Simple(std::mem::take(y))));
            }
            (OpenBracket, Simple, CloseBracket, _) => {
                let Some(c) = expr.pop() else { unreachable!() };
                let Some(b) = expr.pop() else { unreachable!() };
                let Some(a) = expr.pop() else { unreachable!() };
                expr.push(b);
            }
            (OpenBracket, Parallel | Sequential | Race, CloseBracket, _) => {
                let Some(c) = expr.pop() else { unreachable!() };
                let Some(Thing::Composed(b)) = expr.pop() else {
                    unreachable!()
                };
                let Some(a) = expr.pop() else { unreachable!() };
                expr.push(Thing::BracketedExpression(b));
            }
            (Simple | Bracketed, Ampersand, Simple | Bracketed, _) => {
                let Some(Thing::Composed(c) | Thing::BracketedExpression(c)) = expr.pop() else {
                    unreachable!()
                };
                let Some(b) = expr.pop() else { unreachable!() };
                let Some(Thing::Composed(a) | Thing::BracketedExpression(a)) = expr.pop() else {
                    unreachable!()
                };
                expr.push(Thing::Composed(ComposedArgument::Parallel(vec![
                    Box::new(a),
                    Box::new(c),
                ])));
            }
            (Simple | Bracketed, Semicolon, Simple | Bracketed, _) => {
                let Some(Thing::Composed(c) | Thing::BracketedExpression(c)) = expr.pop() else {
                    unreachable!()
                };
                let Some(b) = expr.pop() else { unreachable!() };
                let Some(Thing::Composed(a) | Thing::BracketedExpression(a)) = expr.pop() else {
                    unreachable!()
                };
                expr.push(Thing::Composed(ComposedArgument::Sequential(vec![
                    Box::new(a),
                    Box::new(c),
                ])));
            }
            (Simple | Bracketed, Circumflex, Simple | Bracketed, _) => {
                let Some(Thing::Composed(c) | Thing::BracketedExpression(c)) = expr.pop() else {
                    unreachable!()
                };
                let Some(b) = expr.pop() else { unreachable!() };
                let Some(Thing::Composed(a) | Thing::BracketedExpression(a)) = expr.pop() else {
                    unreachable!()
                };
                expr.push(Thing::Composed(ComposedArgument::Race(vec![
                    Box::new(a),
                    Box::new(c),
                ])));
            }
            (Parallel, Ampersand, Simple | Bracketed, _) => {
                let Some(Thing::Composed(c) | Thing::BracketedExpression(c)) = expr.pop() else {
                    unreachable!()
                };
                let Some(b) = expr.pop() else { unreachable!() };
                let Some(Thing::Composed(ComposedArgument::Parallel(mut a))) = expr.pop() else {
                    unreachable!()
                };
                a.push(Box::new(c));
                expr.push(Thing::Composed(ComposedArgument::Parallel(a)));
            }
            (Sequential, Semicolon, Simple | Bracketed, _) => {
                let Some(Thing::Composed(c) | Thing::BracketedExpression(c)) = expr.pop() else {
                    unreachable!()
                };
                let Some(b) = expr.pop() else { unreachable!() };
                let Some(Thing::Composed(ComposedArgument::Sequential(mut a))) = expr.pop() else {
                    unreachable!()
                };
                a.push(Box::new(c));
                expr.push(Thing::Composed(ComposedArgument::Sequential(a)));
            }
            (Race, Circumflex, Simple | Bracketed, _) => {
                let Some(Thing::Composed(c) | Thing::BracketedExpression(c)) = expr.pop() else {
                    unreachable!()
                };
                let Some(b) = expr.pop() else { unreachable!() };
                let Some(Thing::Composed(ComposedArgument::Race(mut a))) = expr.pop() else {
                    unreachable!()
                };
                a.push(Box::new(c));
                expr.push(Thing::Composed(ComposedArgument::Race(a)));
            }
            (
                Race | Parallel | Sequential,
                Semicolon | Circumflex | Ampersand,
                Simple | Bracketed,
                _,
            ) => {
                anyhow::bail!("Cannot naively mix different operations in --compose mode. Use parentheses to specify priority explicitly.")
            }
            (Empty, Empty, Simple | Parallel | Sequential | Race, false) => {
                let Some(Thing::Composed(x)) = expr.pop() else {
                    unreachable!()
                };
                return Ok(x);
            }
            (Empty, Empty, Bracketed, false) => {
                let Some(Thing::BracketedExpression(x)) = expr.pop() else {
                    unreachable!()
                };
                expr.push(Thing::Composed(x));
            }
            (_, _, _, true) => {
                expr.push(Thing::Terminal(current_event));
                current_event += 1;
            }
            (a, b, c, d) => {
                anyhow::bail!("Invalid composed command line: {a:?},{b:?},{c:?} moreevents={d}");
            }
        }
    }
}

#[test]
fn test_parse2() {
    assert_eq!(
        parse(vec!["qqq"]).unwrap(),
        ComposedArgument::Simple(vec!["w".into(), "qqq".into()])
    );
    assert_eq!(
        parse(vec!["(", "qqq", ")"]).unwrap(),
        ComposedArgument::Simple(vec!["w".into(), "qqq".into()])
    );
    assert_eq!(
        parse(vec!["(", "(", "qqq", "www", ")", ")"]).unwrap(),
        ComposedArgument::Simple(vec!["w".into(), "qqq".into(), "www".into()])
    );
}

#[test]
fn test_parse3() {
    assert_eq!(
        parse(vec!["qqq", "&", "www"]).unwrap(),
        ComposedArgument::Parallel(vec![
            Box::new(ComposedArgument::Simple(vec!["w".into(), "qqq".into()])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "www".into()]))
        ])
    );
    assert_eq!(
        parse(vec!["qqq", "&", "www", "&", "eee"]).unwrap(),
        ComposedArgument::Parallel(vec![
            Box::new(ComposedArgument::Simple(vec!["w".into(), "qqq".into()])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "www".into()])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "eee".into()])),
        ])
    );
    assert_eq!(
        parse(vec!["qqq", ";", "www"]).unwrap(),
        ComposedArgument::Sequential(vec![
            Box::new(ComposedArgument::Simple(vec!["w".into(), "qqq".into()])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "www".into()]))
        ])
    );
    assert_eq!(
        parse(vec!["qqq", ";", "www", ";", "eee"]).unwrap(),
        ComposedArgument::Sequential(vec![
            Box::new(ComposedArgument::Simple(vec!["w".into(), "qqq".into()])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "www".into()])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "eee".into()])),
        ])
    );
    assert_eq!(
        parse(vec!["qqq", "^", "www"]).unwrap(),
        ComposedArgument::Race(vec![
            Box::new(ComposedArgument::Simple(vec!["w".into(), "qqq".into()])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "www".into()]))
        ])
    );
    assert_eq!(
        parse(vec!["qqq", "^", "www", "^", "eee"]).unwrap(),
        ComposedArgument::Race(vec![
            Box::new(ComposedArgument::Simple(vec!["w".into(), "qqq".into()])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "www".into()])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "eee".into()])),
        ])
    );
}

#[test]
fn test_parse4() {
    assert_eq!(
        parse(vec!["(", "qqq", "&", "www", ")"]).unwrap(),
        ComposedArgument::Parallel(vec![
            Box::new(ComposedArgument::Simple(vec!["w".into(), "qqq".into()])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "www".into()]))
        ])
    );
    assert_eq!(
        parse(vec!["(", "qqq", ")", "&", "(", "www", ")"]).unwrap(),
        ComposedArgument::Parallel(vec![
            Box::new(ComposedArgument::Simple(vec!["w".into(), "qqq".into()])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "www".into()]))
        ])
    );
    assert_eq!(
        parse(vec!["(", "(", "qqq", ")", ";", "(", "www", ")", ")"]).unwrap(),
        ComposedArgument::Sequential(vec![
            Box::new(ComposedArgument::Simple(vec!["w".into(), "qqq".into()])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "www".into()]))
        ])
    );
}
#[test]
fn test_parse5() {
    assert_eq!(
        parse(vec!["(", "qqq", "&", "www", ")", ";", "eee"]).unwrap(),
        ComposedArgument::Sequential(vec![
            Box::new(ComposedArgument::Parallel(vec![
                Box::new(ComposedArgument::Simple(vec!["w".into(), "qqq".into()])),
                Box::new(ComposedArgument::Simple(vec!["w".into(), "www".into()]))
            ])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "eee".into()])),
        ]),
    );
    assert_eq!(
        parse(vec!["qqq", "&", "(", "www", ";", "eee", ")"]).unwrap(),
        ComposedArgument::Parallel(vec![
            Box::new(ComposedArgument::Simple(vec!["w".into(), "qqq".into()])),
            Box::new(ComposedArgument::Sequential(vec![
                Box::new(ComposedArgument::Simple(vec!["w".into(), "www".into()])),
                Box::new(ComposedArgument::Simple(vec!["w".into(), "eee".into()]))
            ])),
        ]),
    );
    assert_eq!(
        parse(vec![
            "ppp", "&", "(", "(", "qqq", ")", ")", "&", "(", "(", "www", "^", "sss", "^", "ttt",
            ")", ";", "eee", ";", "ooo", ")"
        ])
        .unwrap(),
        ComposedArgument::Parallel(vec![
            Box::new(ComposedArgument::Simple(vec!["w".into(), "ppp".into()])),
            Box::new(ComposedArgument::Simple(vec!["w".into(), "qqq".into()])),
            Box::new(ComposedArgument::Sequential(vec![
                Box::new(ComposedArgument::Race(vec![
                    Box::new(ComposedArgument::Simple(vec!["w".into(), "www".into()])),
                    Box::new(ComposedArgument::Simple(vec!["w".into(), "sss".into()])),
                    Box::new(ComposedArgument::Simple(vec!["w".into(), "ttt".into()])),
                ])),
                Box::new(ComposedArgument::Simple(vec!["w".into(), "eee".into()])),
                Box::new(ComposedArgument::Simple(vec!["w".into(), "ooo".into()])),
            ])),
        ]),
    );
}
#[test]
fn test_parse_err() {
    parse(vec!["(", "qqq"]).unwrap_err();
    parse(vec!["("]).unwrap_err();
    parse(vec![")"]).unwrap_err();
    parse(vec!["&"]).unwrap_err();
    parse(vec!["qqq", ";"]).unwrap_err();
    parse(vec!["^", "qqq"]).unwrap_err();
    parse(vec!["www", "^", "qqq", "&", "eee"]).unwrap_err();
    parse(vec!["www", ";", "qqq", "&", "eee"]).unwrap_err();
    parse(vec!["www", "&", "qqq", ";", "eee"]).unwrap_err();
}
