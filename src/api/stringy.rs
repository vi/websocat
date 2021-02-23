use anyhow::Context;
use std::collections::HashMap;

use super::Result;

#[derive(Eq, PartialEq)]
pub enum StringOrSubnode {
    Str(String),
    Subnode(StringyNode),
}
/// A part of parsed command line before looking up the SpecifierClasses.
#[derive(Eq, PartialEq)]
pub struct StringyNode {
    pub name: String,
    pub properties: Vec<(String, StringOrSubnode)>,
    pub array: Vec<StringOrSubnode>,
    // pub child_nodes: id_tree::NodeId -- implied,
}


struct ValueForPrinting<'a>(&'a str);

impl<'a> std::fmt::Display for ValueForPrinting<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::with_capacity(self.0.len());
        let mut tainted = false;
        for x in self.0.as_bytes().iter().map(|b|std::ascii::escape_default(*b)) {
            let mut x : Vec<u8> = x.collect();
            
            if x.len() > 1 { tainted = true; }
            if x[0] == b':' || x[0] == b',' || x[0] == b' ' { tainted = true; }

            if x[0] == b'[' { tainted = true;  }
            if x[0] == b']' { tainted = true;  }
            if x[0] == b'=' { tainted = true;  }

            s.push_str(&String::from_utf8(x).unwrap());
        }
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
            match v {
                StringOrSubnode::Str(x) => write!(f, " {}={}", k, ValueForPrinting(x)),
                StringOrSubnode::Subnode(x) => write!(f, " {}={}", k, x),
            };
        }
        for e in &self.array {
            match e {
                StringOrSubnode::Str(x) => write!(f, " {}", ValueForPrinting(x)),
                StringOrSubnode::Subnode(x) => write!(f, " {}", x),
            };
        }
        write!(f, "]")?;
        Ok(())
    }
}

mod tests;

#[rustfmt::skip] // tends to collapse character ranges into one line and to remove trailing `|`s.
impl StringyNode {
    fn read(r: &mut std::iter::Peekable<impl Iterator<Item=u8>>) -> Result<StringyNode> {
        let mut chunk : Vec<u8> = Vec::with_capacity(20);

        if r.next() != Some(b'[') { anyhow::bail!("Tree node must begin with `[` character"); }

        #[derive(Clone,Copy, Eq, PartialEq, Debug)]
        enum S {
            BeforeName,
            Name,
            ForcedSpace,
            Space,
            Chunk,
            ChunkEsc,
            ChunkEscBs,
            ChunkEscHex,
            Finish,
        }

        let mut state = S::BeforeName;

        let mut name : Option<String> = None;
        let mut array: Vec<StringOrSubnode> = vec![];
        let mut properties: Vec<(String, StringOrSubnode)> = vec![];
    
        let mut property_name : Option<String> = None;

        let mut hex : tinyvec::ArrayVec<[u8; 2]> = Default::default();

        while let Some(c) = r.peek() {
            //eprintln!("{:?} {}", state, c);
            match state {
                S::Name | S::BeforeName => {
                    match c {
                        | b'0'..=b'9'
                        | b'a'..=b'z'
                        | b'A'..=b'Z'
                        | b'_'
                        | b'.'
                        | b'\x80' ..= b'\xFF'
                        => {
                            chunk.push(*c);
                            state = S::Name;
                        }
                        b' ' => {
                            if state == S::Name {
                                name = Some(String::from_utf8(chunk)?);
                                chunk = Vec::with_capacity(20);
                                state = S::Space;
                            } else {
                                // no-op
                            }
                        }
                        b']' => {
                            if state == S::Name {
                                name = Some(String::from_utf8(chunk)?); 
                            } 
                            state = S::Finish;
                            r.next();
                            break;
                        }
                        _ => anyhow::bail!("Invalid character {} while reading tree node name", std::ascii::escape_default(*c)),
                    }
                }
                S::ForcedSpace => {
                    match c {
                        b' ' => {
                            state = S::Space;
                        }
                        b']' => {
                            state = S::Finish;
                            r.next();
                            break;
                        }
                        _ => anyhow::bail!(
                            "Expected a space character or `]` after `\"` or `]`, not {} when parsing node named {}",
                            std::ascii::escape_default(*c),
                            name.as_ref().map(|x|&**x).unwrap_or("???"),
                        ),
                    }
                }
                S::Space => {
                    match c {
                        | b'0'..=b'9'
                        | b'a'..=b'z'
                        | b'A'..=b'Z'
                        | b'_'
                        | b'.'
                        | b'\x80' ..= b'\xFF'
                        => {
                            chunk.push(*c);
                            state = S::Chunk;
                        }
                        b'"' => {
                            state = S::ChunkEsc;
                        }
                        b']' => {
                            r.next();
                            state = S::Finish;
                            break;
                        }
                        b'[' => {
                            let subnode = StringyNode::read(r).with_context(||format!(
                                "Failed to read subnode array element {} of node {}",
                                array.len()+1,
                                name.as_ref().map(|x|&**x).unwrap_or("???"),
                            ))?;
                            array.push(StringOrSubnode::Subnode(subnode));
                            state = S::ForcedSpace;
                            continue;
                        }
                        b' ' => {
                            // no-op
                        }
                        _ => anyhow::bail!(
                            "Invalid character {} in tree node named {}",
                            std::ascii::escape_default(*c),
                            name.as_ref().map(|x|&**x).unwrap_or("???")
                        ),
                    }
                }
                S::Chunk => {
                    match c {
                        | b'0'..=b'9'
                        | b'a'..=b'z'
                        | b'A'..=b'Z'
                        | b'_'
                        | b'.'
                        | b'\x80' ..= b'\xFF'
                        => {
                            chunk.push(*c);
                        }
                        b' ' | b']' => {
                            if chunk.is_empty() {
                                anyhow::bail!(
                                    "Unescaped empty propery {} value of tree node {}",
                                    property_name.as_ref().map(|x|&**x).unwrap_or("???"),
                                    name.as_ref().map(|x|&**x).unwrap_or("???")
                                );
                            }
                            let ch = String::from_utf8(chunk)?;
                            chunk = Vec::with_capacity(20);
                            if let Some(pn) = property_name {
                                properties.push((pn, StringOrSubnode::Str(ch)));
                            } else {
                                array.push(StringOrSubnode::Str(ch));
                            }
                            property_name = None;
                            if *c == b']' {
                                state = S::Finish;
                                r.next();
                                break;
                            } else {
                                state = S::Space;
                            }
                        }
                        b'=' => {
                            if property_name.is_some() {
                                anyhow::bail!(
                                    "Duplicate unescaped = character when paring property {} of a tree node {}",
                                    property_name.unwrap(),
                                    name.as_ref().map(|x|&**x).unwrap_or("???")
                                );
                            }
                            let ch = String::from_utf8(chunk)?;
                            property_name = Some(ch);
                            chunk = Vec::with_capacity(20);
                        }
                        b'"' => {
                            if property_name.is_none() || ! chunk.is_empty() {
                                anyhow::bail!(
                                    "Property value `\"` escape character in a tree node named {} must come immediately after `=`",
                                    name.as_ref().map(|x|&**x).unwrap_or("???")
                                );
                            }
                            state = S::ChunkEsc;
                        }
                        b'[' => {
                            if let Some(pn) = property_name {
                                if ! chunk.is_empty() {
                                    anyhow::bail!(
                                        "Wrong `[` character position when parsing a tree node named {}",
                                        name.as_ref().map(|x|&**x).unwrap_or("???")
                                    );
                                }
                                let subnode = StringyNode::read(r).with_context(||format!(
                                    "Failed to read property {} value of node {}",
                                    pn,
                                    name.as_ref().map(|x|&**x).unwrap_or("???"),
                                ))?;
                                properties.push((pn, StringOrSubnode::Subnode(subnode)));
                                state = S::ForcedSpace;
                                property_name = None;
                                continue;
                            } else {
                                anyhow::bail!(
                                    "Wrong `[` character position when parsing a tree node named {}",
                                    name.as_ref().map(|x|&**x).unwrap_or("???")
                                );
                            }
                        }
                        _ => anyhow::bail!(
                            "Invalid character {} in tree node named {} when a parsing potential property or array element",
                            std::ascii::escape_default(*c),
                            name.as_ref().map(|x|&**x).unwrap_or("???")
                        ),
                    }
                }
                S::ChunkEsc => {
                    match c {
                        b'"' => {
                            let ch = String::from_utf8(chunk)?;
                            chunk = Vec::with_capacity(20);
                            if let Some(pn) = property_name {
                                properties.push((pn, StringOrSubnode::Str(ch)));
                            } else {
                                array.push(StringOrSubnode::Str(ch));
                            }
                            property_name = None;
                            state = S::ForcedSpace;
                        }
                        b'\\' => {
                            state = S::ChunkEscBs;
                        }
                        _ => {
                            chunk.push(*c);
                        }
                    }
                }
                S::ChunkEscBs => {
                    match c {
                        b't' => chunk.push(b'\t'),
                        b'n' => chunk.push(b'\n'),
                        b'\'' => chunk.push(b'\''),
                        b'"' => chunk.push(b'"'),
                        b'\\' => chunk.push(b'\\'),
                        b'x' => (),
                        _ => anyhow::bail!(
                            "Invalid escape sequence character {} when parsing tree node {}",
                            std::ascii::escape_default(*c),
                            name.as_ref().map(|x|&**x).unwrap_or("???"),
                        ),
                    }
                    state = S::ChunkEsc;
                    if *c == b'x' { state = S::ChunkEscHex; }
                }
                S::ChunkEscHex => {
                    match c {
                        b'0' ..= b'9' => hex.push(*c - b'0'),
                        b'a' ..= b'f' => hex.push(*c + 10 - b'a'),
                        b'A' ..= b'F' => hex.push(*c + 10 - b'A'),
                        _ => anyhow::bail!(
                            "Invalid hex escape sequence character {} when parsing tree node {}",
                            std::ascii::escape_default(*c),
                            name.as_ref().map(|x|&**x).unwrap_or("???"),
                        ),
                    }
                    if hex.len() == 2 {
                        chunk.push(hex[0] * 16 + hex[1]);
                        state = S::ChunkEsc;
                        hex.clear();
                    }
                }
                S::Finish => break,
            }
            r.next();
        }
        if state != S::Finish {
            anyhow::bail!(
                "Trimmed input when parsing the tree node named {}",
                name.as_ref().map(|x|&**x).unwrap_or("???"),
            );
        }
        if name.is_none() {
            anyhow::bail!(
                "Empty tree nodes are not allowed",
            );
        }

        Ok(StringyNode {
            name: name.unwrap(),
            properties,
            array,
        })
    }
}

impl std::str::FromStr for StringyNode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        StringyNode::read(&mut s.bytes().peekable())
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

            use super::{PropertyValueType as PVT, PropertyValue as PV};
            use StringOrSubnode::{Str,Subnode};

            for (k, v) in &self.properties {
                if let Some(typ) = p.get(k) {
                    let vv = match (typ, v) {
                        (PVT::ChildNode, Subnode(x)) => PV::ChildNode(
                            x.build_impl(classes_by_prefix, tree)?,
                        ),
                        (ty, Str(x)) => ty.interpret(x).with_context(|| {
                            format!(
                                "Failed to parse property {} in node {} that has value `{}`",
                                k, self.name, x
                            )
                        })?,
                        (PVT::ChildNode, _) => anyhow::bail!("Subnode (`[...]`) expected as a property value {} of node {}", k, self.name),
                        (_, Subnode(_)) => anyhow::bail!("A subnode is not expected as a property value {} of node {}", k, self.name),
                    };
                    b.set_property(k, vv).with_context(|| {
                        format!("Failed to set property {} in node {}", k, self.name)
                    })?;
                } else {
                    anyhow::bail!("Property {} of node type {} not found", k, self.name);
                }
            }

            let at = cls.array_type();

            for (n, e) in self.array.iter().enumerate() {
                if let Some(at) = &at {
                    let vv = match (at, e) {
                        (PVT::ChildNode, Subnode(x)) => PV::ChildNode(
                            x.build_impl(classes_by_prefix, tree)?,
                        ),
                        (ty, Str(x)) => ty.interpret(x).with_context(|| {
                            format!(
                                "Failed to array element number {} in node {} that has value `{}`",
                                n, self.name, x
                            )
                        })?,
                        (PVT::ChildNode, _) => anyhow::bail!("Subnode (`[...]`) expected as an array element number {} of node {}", n, self.name),
                        (_, Subnode(_)) => anyhow::bail!("A subnode is not expected as an array element number {} of node {}", n, self.name),
                    };

                    b.push_array_element(vv)
                    .with_context(|| {
                        format!("Failed to push array element number `{}` to node {}", n, self.name)
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
