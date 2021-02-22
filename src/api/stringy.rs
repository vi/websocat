use anyhow::Context;
use std::collections::HashMap;

use super::Result;

/// A part of parsed command line before looking up the SpecifierClasses.
pub struct StringyNode {
    pub name: String,
    pub properties: HashMap<String, String>,
    pub array: Vec<String>,
    // pub child_nodes: id_tree::NodeId -- implied,
}


struct ValueForPrinting<'a>(&'a str);

impl<'a> std::fmt::Display for ValueForPrinting<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::with_capacity(self.0.len());
        let mut tainted = false;
        let mut balance : i32 = 0;
        for x in self.0.as_bytes().iter().map(|b|std::ascii::escape_default(*b)) {
            let x : Vec<u8> = x.collect();
            
            if balance == 0 {
                if x.len() > 1 { tainted = true; }
                if x[0] == b':' || x[0] == b',' || x[0] == b' ' { tainted = true; }
            }

            if x[0] == b'[' { balance += 1; }
            if x[0] == b']' { balance -= 1; }
            if balance < 0 { tainted = true; }

            s.push_str(&String::from_utf8(x).unwrap());
        }
        if balance != 0 { tainted = true; }
        if self.0.len() == 0 { tainted = true; }
        if tainted {
            write!(f, "\"{}\"", s);
        } else {
            write!(f, "{}", self.0)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for StringyNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}", self.name)?;
        for (k, v) in &self.properties {
            write!(f, " {}={}", k, ValueForPrinting(v));
        }
        for e in &self.array {
            write!(f, " {}", ValueForPrinting(e));
        }
        write!(f, "]")?;
        Ok(())
    }
}

#[test]
fn test_display1() {
    assert_eq!(format!("{}", StringyNode {
        name: "qqq".to_owned(),
        properties: HashMap::new(),
        array: Vec::new(),
    }), "[qqq]");

    assert_eq!(format!("{}", StringyNode {
        name: "www".to_owned(),
        properties: vec![("a".to_owned(),"b".to_owned())].into_iter().collect(),
        array: Vec::new(),
    }), "[www a=b]");

    assert_eq!(format!("{}", StringyNode {
        name: "eee".to_owned(),
        properties: HashMap::new(),
        array: vec!["c".to_owned()],
    }), "[eee c]");

    assert_eq!(format!("{}", StringyNode {
        name: "rrr".to_owned(),
        properties: vec![("a".to_owned(),"b".to_owned())].into_iter().collect(),
        array: vec!["c".to_owned()],
    }), "[rrr a=b c]");

    assert_eq!(format!("{}", StringyNode {
        name: "ttt".to_owned(),
        properties: vec![("a".to_owned(),"b".to_owned()), ("a2".to_owned(),"b2".to_owned())].into_iter().collect(),
        array: vec!["c".to_owned(), "c2".to_owned()],
    }), "[ttt a=b a2=b2 c c2]");
}
#[test]
fn test_display2() {
    assert_eq!(format!("{}", StringyNode {
        name: "eee".to_owned(),
        properties: HashMap::new(),
        array: vec!["\"".to_owned()],
    }), "[eee \"\\\"\"]");

    assert_eq!(format!("{}", StringyNode {
        name: "eee".to_owned(),
        properties: HashMap::new(),
        array: vec!["[]".to_owned()],
    }), "[eee []]");

    assert_eq!(format!("{}", StringyNode {
        name: "eee".to_owned(),
        properties: HashMap::new(),
        array: vec!["[".to_owned()],
    }), "[eee \"[\"]");

    assert_eq!(format!("{}", StringyNode {
        name: "eee".to_owned(),
        properties: HashMap::new(),
        array: vec!["".to_owned()],
    }), "[eee \"\"]");

    assert_eq!(format!("{}", StringyNode {
        name: "eee".to_owned(),
        properties: HashMap::new(),
        array: vec!["]".to_owned()],
    }), "[eee \"]\"]");

    assert_eq!(format!("{}", StringyNode {
        name: "eee".to_owned(),
        properties: HashMap::new(),
        array: vec!["\\".to_owned()],
    }), "[eee \"\\\\\"]");


    assert_eq!(format!("{}", StringyNode {
        name: "eee".to_owned(),
        properties: HashMap::new(),
        array: vec!["[qqq w=e r \"\"]".to_owned()],
    }), "[eee [qqq w=e r \"\"]]");
}

impl std::str::FromStr for StringyNode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut n = StringyNode {
            name: String::with_capacity(20),
            properties: HashMap::new(),
            array: Vec::new(),
        };

        todo!();

        Ok(n)
    }
}

impl super::PropertyValueType {
    pub fn interpret(&self, x: &str) -> super::Result<super::PropertyValue> {
        use super::{PropertyValue as PV, PropertyValueType as PVT};
        match self {
            PVT::Stringy => Ok(PV::Stringy(x.to_owned())),
            PVT::Enummy(_) => todo!(),
            PVT::Numbery => todo!(),
            PVT::Floaty => todo!(),
            PVT::Booly => todo!(),
            PVT::SockAddr => todo!(),
            PVT::IpAddr => todo!(),
            PVT::PortNumber => todo!(),
            PVT::Path => todo!(),
            PVT::Uri => todo!(),
            PVT::Duration => todo!(),
            PVT::ChildNode => panic!(
                "You can't use PropertyValueType::interpret for obtaining child node pointers"
            ),
        }
    }
}

impl StringyNode {
    fn build_impl(
        &self,
        classes_by_prefix: &HashMap<String, super::DNodeClass>,
        tree: &mut super::Slab<super::NodeId, super::DParsedNode>,
    ) -> Result<super::NodeId> {
        if let Some(cls) = classes_by_prefix.get(&self.name) {
            let props = cls.properties();
            let mut p: HashMap<String, super::PropertyValueType> =
                HashMap::with_capacity(props.len());
            p.extend(props.into_iter().map(|pi| (pi.name, pi.r#type)));

            let mut b = cls.new_node();

            for (k, v) in &self.properties {
                if let Some(typ) = p.get(k) {
                    let vv = match typ {
                        super::PropertyValueType::ChildNode => super::PropertyValue::ChildNode(
                            self.build_impl(classes_by_prefix, tree)?,
                        ),
                        ty => ty.interpret(v).with_context(|| {
                            format!(
                                "Failed to parse property {} in node {} that has value `{}`",
                                k, self.name, v
                            )
                        })?,
                    };
                    b.set_property(k, vv).with_context(|| {
                        format!("Failed to set property {} in node {}", k, self.name)
                    })?;
                } else {
                    anyhow::bail!("Property {} of node type {} not found", k, self.name);
                }
            }

            let at = cls.array_type();

            for e in &self.array {
                if let Some(at) = &at {
                    b.push_array_element(at.interpret(e).with_context(|| {
                        format!(
                            "Failed to parse array element `{}` in node {}",
                            e, self.name
                        )
                    })?)
                    .with_context(|| {
                        format!("Failed to push array element `{}` to node {}", e, self.name)
                    })?;
                } else {
                    anyhow::bail!("Node type {} does not support array elements", self.name);
                }
            }

            let mut node = tree.vacant_entry();
            let key = node.key();
            node.insert(todo!());
            Ok(key)
        } else {
            anyhow::bail!("Node type {} not found", self.name)
        }
    }

    pub fn build(
        &self,
        classes_by_prefix: &HashMap<String, super::DNodeClass>,
        tree: &mut super::Slab<super::NodeId, super::DParsedNode>,
    ) -> Result<super::NodeId> {
        self.build_impl(classes_by_prefix, tree)
    }
}
