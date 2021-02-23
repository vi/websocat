#![cfg(test)]

use std::str::FromStr;

use super::*;

use StringOrSubnode::{Str, Subnode};

fn ss(x: &'static str) -> StringOrSubnode {
    Str(x.to_owned())
}
fn s(x: &'static str) -> Ident {
    Ident(x.to_owned())
}

#[test]
fn test_display_simple() {
    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("qqq"),
                properties: vec![],
                array: Vec::new(),
            }
        ),
        "[qqq]"
    );

    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("www"),
                properties: vec![(s("a"), ss("b"))].into_iter().collect(),
                array: Vec::new(),
            }
        ),
        "[www a=b]"
    );

    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("eee"),
                properties: vec![],
                array: vec![ss("c")],
            }
        ),
        "[eee c]"
    );

    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("rrr"),
                properties: vec![(s("a"), ss("b"))].into_iter().collect(),
                array: vec![Str("c".to_owned())],
            }
        ),
        "[rrr a=b c]"
    );

    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("ttt"),
                properties: vec![(s("a"), ss("b")), (s("a2"), ss("b2"))]
                    .into_iter()
                    .collect(),
                array: vec![ss("c"), ss("c2")],
            }
        ),
        "[ttt a=b a2=b2 c c2]"
    );
}
#[test]
fn test_display_escaping() {
    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("eee"),
                properties: vec![],
                array: vec![ss("\"")],
            }
        ),
        "[eee \"\\\"\"]"
    );

    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("eee"),
                properties: vec![],
                array: vec![ss("[]")],
            }
        ),
        "[eee \"[]\"]"
    );

    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("eee"),
                properties: vec![],
                array: vec![ss("[")],
            }
        ),
        "[eee \"[\"]"
    );

    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("eee"),
                properties: vec![],
                array: vec![ss("")],
            }
        ),
        "[eee \"\"]"
    );

    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("eee"),
                properties: vec![],
                array: vec![ss("]")],
            }
        ),
        "[eee \"]\"]"
    );

    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("eee"),
                properties: vec![],
                array: vec![ss("a=b")],
            }
        ),
        "[eee \"a=b\"]"
    );

    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("eee"),
                properties: vec![],
                array: vec![ss("\\")],
            }
        ),
        "[eee \"\\\\\"]"
    );

    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("eee"),
                properties: vec![],
                array: vec![ss(r#"[qqq w=e r ""]"#)],
            }
        ),
        r#"[eee "[qqq w=e r \"\"]"]"#
    );
}

#[test]
fn test_display_nested() {
    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("eee"),
                properties: vec![(
                    s("zz"),
                    Subnode(StringyNode {
                        name: s("bobo"),
                        properties: vec![],
                        array: vec![],
                    })
                )],
                array: vec![],
            }
        ),
        "[eee zz=[bobo]]"
    );

    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("rrr"),
                properties: vec![(s("zz"), ss("z"))],
                array: vec![Subnode(StringyNode {
                    name: s("xoxo"),
                    properties: vec![],
                    array: vec![],
                })],
            }
        ),
        "[rrr zz=z [xoxo]]"
    );

    assert_eq!(
        format!(
            "{}",
            StringyNode {
                name: s("ttt"),
                properties: vec![
                    (s("zz"), ss("z")),
                    (s("zz2"), ss("z2")),
                    (s("zz3"), ss("[qq]")),
                    (
                        s("inner"),
                        Subnode(StringyNode {
                            name: s("yoyo"),
                            properties: vec![(
                                s("kk"),
                                Subnode(StringyNode {
                                    name: s("yoyo"),
                                    properties: vec![(s("mm"), ss("MMM"))],
                                    array: vec![],
                                })
                            )],
                            array: vec![],
                        })
                    ),
                    (s("zz4"), ss("5.6")),
                ],
                array: vec![Subnode(StringyNode {
                    name: s("ppp"),
                    properties: vec![],
                    array: vec![Subnode(StringyNode {
                        name: s("pp"),
                        properties: vec![],
                        array: vec![Subnode(StringyNode {
                            name: s("p"),
                            properties: vec![],
                            array: vec![],
                        })],
                    })],
                })],
            }
        ),
        r#"[ttt zz=z zz2=z2 zz3="[qq]" inner=[yoyo kk=[yoyo mm=MMM]] zz4=5.6 [ppp [pp [p]]]]"#
    );
}


fn pass(x: &'static str, y: &'static str) {
    assert_eq!(StringyNode::from_str(x).ok().map(|x|format!("{}", x)), Some(y.to_owned()));
}

fn fail(x: &'static str) {
    assert_eq!(StringyNode::from_str(x).ok().map(|x|format!("{}", x)), None);
}
#[test]
fn test_parse1() {
    fail("qwe");
    fail("[]");
    fail("[");
    fail("[qqq");
    fail("[qqq w=]");
    fail("[qqq w");
    fail("[qqq w[yyy]]");
    fail("[qqq w=e[yyy]]");
    fail("[qqq [y][w]]");
    fail(r#"[qqq "rt\l"]"#);
    fail(r#"[qqq "rt\x"]"#);
    fail(r#"[qqq "rt\x2"]"#);
    fail(r#"[qqq "rt\xPP"]"#);
    fail(r#"[qqq "rt\xP"#);

    pass("[qqq]", "[qqq]");
    pass("[   qqq   ]", "[qqq]");
    pass("[  qqq  a=2 b=4 ]", "[qqq a=2 b=4]");
    pass("[qqq [w [r]]]", "[qqq [w [r]]]");
    pass("[qqq 55 y=7]", "[qqq y=7 55]");
    pass("[qqq inner=[b]]", "[qqq inner=[b]]");


    pass(r#"[qqq ""]"#, r#"[qqq ""]"#);
    pass(r#"[qqq "\\"]"#, r#"[qqq "\\"]"#);
    pass(r#"[qqq "\x20\n"]"#, r#"[qqq " \n"]"#);
}

#[derive(Debug)]
pub struct SoSStrat;


type SoST = < <super::StringyNode as proptest::arbitrary::Arbitrary>::Strategy as proptest::strategy::Strategy>::Tree;
type SsST = < <String as proptest::arbitrary::Arbitrary>::Strategy as proptest::strategy::Strategy>::Tree;
pub enum SoSTree {
    Sub(SoST),
    Str(SsST),
}

impl proptest::arbitrary::Arbitrary for super::StringOrSubnode {
    type Parameters = ();
    type Strategy = SoSStrat;
    fn arbitrary_with(args: Self::Parameters) -> Self::Strategy {
        SoSStrat
    }
}

impl proptest::strategy::Strategy for SoSStrat {
    type Value = super::StringOrSubnode;
    type Tree = SoSTree;

    fn new_tree(&self, runner: &mut proptest::test_runner::TestRunner) -> proptest::strategy::NewTree<Self> {
        use proptest::prelude::RngCore;
        use proptest::arbitrary::Arbitrary;
        use proptest::strategy::Strategy;

        let x = runner.rng().next_u32();
        if x % 150 == 0 {
            Ok(SoSTree::Sub(
                super::StringyNode::arbitrary().new_tree(runner)?
            ))
        } else {
            Ok(SoSTree::Str(
                String::arbitrary().new_tree(runner)?
            ))
        }
    }
}

impl proptest::strategy::ValueTree for SoSTree {
    type Value = super::StringOrSubnode;
    fn current(&self) -> Self::Value {
        match self {
            SoSTree::Str(x) => super::StringOrSubnode::Str(x.current()),
            SoSTree::Sub(x) => super::StringOrSubnode::Subnode(x.current()),
        }
    }
    fn simplify(&mut self) -> bool { 
        match self {
            SoSTree::Str(x) => x.simplify(),
            SoSTree::Sub(x) => x.simplify(),
        }
    }
    fn complicate(&mut self) -> bool { 
        match self {
            SoSTree::Str(x) => x.complicate(),
            SoSTree::Sub(x) => x.complicate(),
        }
    }
}

#[test]
#[ignore]
fn print_some_trees() {
    use proptest::arbitrary::Arbitrary;
    use proptest::strategy::{Strategy,ValueTree};

    let mut tr = proptest::test_runner::TestRunner::deterministic();

    for _ in 0..25 {
        let mut q = super::StringyNode::arbitrary().new_tree(&mut tr).unwrap();

        println!("{}", q.current());

        for _ in 0..10 {
            if ! q.simplify() { break; }
            println!("    {}", q.current());
        }
    }
}


use proptest::arbitrary::Arbitrary;

proptest::proptest! {
    #[test]
    fn format_to_read(a in super::StringyNode::arbitrary()) {
        let s = format!("{}", a);
        let b = StringyNode::from_str(&s).unwrap();
        proptest::prop_assert_eq!(a, b);
    }
}
