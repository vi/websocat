#![allow(clippy::unit_arg)]
use anyhow::Context;
use string_interner::Symbol;
use std::collections::HashMap;

use crate::{CliOpts};

use super::Result;

use bytes::{Bytes, Buf, BytesMut, BufMut};

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum StringOrSubnode {
    Str(Bytes),
    Subnode(StrNode),
}
#[derive(Eq, PartialEq, Debug, Clone, Default)]
#[cfg_attr(test,derive(proptest_derive::Arbitrary))]
pub struct Ident(
    #[cfg_attr(test, proptest(regex = "[a-z0-9._]+"))]
    pub String
);

impl From<String> for Ident {
    fn from(x: String) -> Self {
        Ident(x)
    }
}

impl From<&'static str> for Ident {
    fn from(x: &'static str) -> Self {
        Ident(x.to_owned())
    }
}

/// A part of parsed command line before looking up the `NodeClass`es.

#[derive(Eq, PartialEq, Debug, Clone, Default)]
#[cfg_attr(test,derive(proptest_derive::Arbitrary))]
pub struct StrNode {
    pub name: Ident,
    pub properties: Vec<(Ident, StringOrSubnode)>,
    pub array: Vec<StringOrSubnode>,

    /// Allow automatically filling options for this node from CLI arguments
    pub enable_autopopulate: bool,
}


struct ValueForPrinting<T : bytes::Buf>(T);

impl<T: Buf> std::fmt::Display for ValueForPrinting<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::with_capacity(self.0.remaining());
        assert_eq!(self.0.remaining(), self.0.chunk().len());
        let buf = self.0.chunk();

        let mut tainted = false;
        for x in buf.iter().map(|b|std::ascii::escape_default(*b)) {
            let x : Vec<u8> = x.collect();
            
            if x.len() > 1 { 
                tainted = true;
            } else {
                match x.get(0) {
                    Some(x) if identchar(*x) => (),
                    _ => tainted = true,
                }
            }

            s.push_str(&String::from_utf8(x).unwrap());
        }
        if buf.is_empty() { tainted = true; }
        if buf == b"@" { tainted = true; }
        if tainted {
            write!(f, "\"{}\"", s)?;
        } else {
            write!(f, "{}", s)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for StrNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}", self.name.0)?;
        for (k, v) in &self.properties {
            match v {
                StringOrSubnode::Str(x) => write!(f, " {}={}", k.0, ValueForPrinting(x.clone()))?,
                StringOrSubnode::Subnode(x) => write!(f, " {}={}", k.0, x)?,
            };
        }
        for e in &self.array {
            match e {
                StringOrSubnode::Str(x) => write!(f, " {}", ValueForPrinting(x.clone()))?,
                StringOrSubnode::Subnode(x) => write!(f, " {}", x)?,
            };
        }
        if self.enable_autopopulate {
            write!(f, " +]")?;
        } else {
            write!(f, "]")?;
        }
        Ok(())
    }
}

mod tests;

fn identchar(b: u8) -> bool {
    matches!(b, b'0'..=b'9'
        | b'a'..=b'z'
        | b'A'..=b'Z'
        | b'_' | b':' | b'?' | b'@'
        | b'.' | b'/' | b'#' | b'&'
        | b'-'
        | b'\x80' ..= b'\xFF')
}

#[rustfmt::skip] // tends to collapse character ranges into one line and to remove trailing `|`s.
impl StrNode {
    #[tracing::instrument(name="StringyNode::read", level="trace", skip(r,require_opening_bracket), err)]
    fn read(r: &mut std::iter::Peekable<impl Iterator<Item=u8>>, require_opening_bracket: bool) -> Result<StrNode> {
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
        
        tracing::trace!("Reading from string to a stringy node");
        let mut chunk = BytesMut::with_capacity(20);

        if require_opening_bracket {
            if r.next() != Some(b'[') { anyhow::bail!("Tree node must begin with `[` character"); }
        }


        let mut state = S::BeforeName;

        let mut name : Option<String> = None;
        let mut array: Vec<StringOrSubnode> = vec![];
        let mut properties: Vec<(Ident, StringOrSubnode)> = vec![];
    
        let mut property_name : Option<String> = None;

        let mut hex  = tinyvec::ArrayVec::<[u8; 2]>::default();
        
        let mut enable_autopopulate = false;

        while let Some(c) = r.peek() {
            tracing::trace!("Peeked byte {} in state {:?}", std::ascii::escape_default(*c), state);
            //eprintln!("{:?} {}", state, c);
            match state {
                S::Name | S::BeforeName => {
                    match c {
                        x if identchar(*x)
                        => {
                            chunk.put_u8(*c);
                            state = S::Name;
                        }
                        b' ' => {
                            if state == S::Name {
                                name = Some(String::from_utf8(chunk.to_vec())?);
                                chunk = BytesMut::with_capacity(20);
                                state = S::Space;
                            } else {
                                // no-op
                            }
                        }
                        b']' => {
                            if state == S::Name {
                                name = Some(String::from_utf8(chunk.to_vec())?); 
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
                            "Expected a space character or `]` after `\"` or `]` or `+`, not {} when parsing node named {}",
                            std::ascii::escape_default(*c),
                            name.as_deref().unwrap_or("???"),
                        ),
                    }
                }
                S::Space => {
                    match c {
                        b'+' => {
                            if enable_autopopulate {
                                anyhow::bail!("Invalid `+` character: CLI auto-populate is already enabled for this node");
                            }
                            enable_autopopulate = true;
                            state = S::ForcedSpace;
                        }
                        x if identchar(*x)
                        => {
                            chunk.put_u8(*c);
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
                            let subnode = StrNode::read(r, true).with_context(||format!(
                                "Failed to read subnode array element {} of node {}",
                                array.len()+1,
                                name.as_deref().unwrap_or("???"),
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
                            name.as_deref().unwrap_or("???"),
                        ),
                    }
                }
                S::Chunk => {
                    match c {
                        x if identchar(*x)
                        => {
                            chunk.put_u8(*c);
                        }
                        b' ' | b']' => {
                            if chunk.is_empty() {
                                anyhow::bail!(
                                    "Unescaped empty propery {} value of tree node {}",
                                    property_name.as_deref().unwrap_or("???"),
                                    name.as_deref().unwrap_or("???"),
                                );
                            }
                            if chunk == b"@"[..] && property_name.is_none() {
                                if *c == b']' {
                                    anyhow::bail!(
                                        "Invalid `@ ]` combination at the end of node {}",
                                        name.as_deref().unwrap_or("???"),
                                    ); 
                                }
                                let subnode = StrNode::read(r, false).with_context(||format!(
                                    "Failed to read trailing inner subnode element {} of node {}",
                                    array.len()+1,
                                    name.as_deref().unwrap_or("???"),
                                ))?;
                                properties.push((Ident("inner".to_owned()), StringOrSubnode::Subnode(subnode)));
                                state = S::Finish;
                                break;
                            }
                            let ch = chunk;
                            chunk = BytesMut::with_capacity(20);
                            if let Some(pn) = property_name {
                                properties.push((Ident(pn), StringOrSubnode::Str(ch.freeze())));
                            } else {
                                array.push(StringOrSubnode::Str(ch.freeze()));
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
                                    name.as_deref().unwrap_or("???"),
                                );
                            }
                            let ch = String::from_utf8(chunk.to_vec())?;
                            property_name = Some(ch);
                            chunk = BytesMut::with_capacity(20);
                        }
                        b'"' => {
                            if property_name.is_none() || ! chunk.is_empty() {
                                anyhow::bail!(
                                    "Property value `\"` escape character in a tree node named {} must come immediately after `=`",
                                    name.as_deref().unwrap_or("???"),
                                );
                            }
                            state = S::ChunkEsc;
                        }
                        b'[' => {
                            if let Some(pn) = property_name {
                                if ! chunk.is_empty() {
                                    anyhow::bail!(
                                        "Wrong `[` character position when parsing a tree node named {}",
                                        name.as_deref().unwrap_or("???"),
                                    );
                                }
                                let subnode = StrNode::read(r, true).with_context(||format!(
                                    "Failed to read property {} value of node {}",
                                    pn,
                                    name.as_deref().unwrap_or("???"),
                                ))?;
                                properties.push((Ident(pn), StringOrSubnode::Subnode(subnode)));
                                state = S::ForcedSpace;
                                property_name = None;
                                continue;
                            } else {
                                anyhow::bail!(
                                    "Wrong `[` character position when parsing a tree node named {}",
                                    name.as_deref().unwrap_or("???"),
                                );
                            }
                        }
                        _ => anyhow::bail!(
                            "Invalid character {} in tree node named {} when a parsing potential property or array element",
                            std::ascii::escape_default(*c),
                            name.as_deref().unwrap_or("???"),
                        ),
                    }
                }
                S::ChunkEsc => {
                    match c {
                        b'"' => {
                            let ch = chunk;
                            chunk = BytesMut::with_capacity(20);
                            if let Some(pn) = property_name {
                                properties.push((Ident(pn), StringOrSubnode::Str(ch.freeze())));
                            } else {
                                array.push(StringOrSubnode::Str(ch.freeze()));
                            }
                            property_name = None;
                            state = S::ForcedSpace;
                        }
                        b'\\' => {
                            state = S::ChunkEscBs;
                        }
                        _ => {
                            chunk.put_u8(*c);
                        }
                    }
                }
                S::ChunkEscBs => {
                    match c {
                        b't' => chunk.put_u8(b'\t'),
                        b'n' => chunk.put_u8(b'\n'),
                        b'\'' => chunk.put_u8(b'\''),
                        b'"' => chunk.put_u8(b'"'),
                        b'\\' => chunk.put_u8(b'\\'),
                        b'x' => (),
                        _ => anyhow::bail!(
                            "Invalid escape sequence character {} when parsing tree node {}",
                            std::ascii::escape_default(*c),
                            name.as_deref().unwrap_or("???"),
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
                            name.as_deref().unwrap_or("???"),
                        ),
                    }
                    if hex.len() == 2 {
                        chunk.put_u8(hex[0] * 16 + hex[1]);
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
                name.as_deref().unwrap_or("???"),
            );
        }
        if name.is_none() {
            anyhow::bail!(
                "Empty tree nodes are not allowed",
            );
        }
        tracing::debug!("Finished reading from string into stringy node {}", name.as_ref().unwrap());
        Ok(StrNode {
            name: Ident(name.unwrap()),
            properties,
            array,
            enable_autopopulate,
        })
    }
}

impl std::str::FromStr for StrNode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        StrNode::read(&mut s.bytes().peekable(), true)
    }
}

impl StrNode {
    pub fn from_bytes(b: &[u8]) -> Result<Self, anyhow::Error> {
        StrNode::read(&mut b.into_iter().copied().peekable(), true)
    }
}

impl super::PropertyValueType {
    pub fn interpret(&self, b: &Bytes) -> super::Result<super::PropertyValue> {
        use super::{PropertyValue as PV, PropertyValueType as PVT};
        if matches!(self, PVT::BytesBuffer) {
            return Ok(PV::BytesBuffer(b.clone()));
        }
        let x = String::from_utf8(b.to_vec())?;
        match self {
            PVT::Stringy => Ok(PV::Stringy(x)),
            PVT::OsString => Ok(PV::OsString(x.into())),
            PVT::BytesBuffer => unreachable!(),
            PVT::Enummy(si) => {
                if let Some(sym) = si.get(&x) {
                    Ok(PV::Enummy(sym))
                } else {
                    let totallen : usize  = si.into_iter().map(|(_,v)|v.len()+1).sum();
                    let mut valids = String::with_capacity(totallen);
                    for (_, v) in si {
                        if x.to_lowercase() == v.to_lowercase() {
                            anyhow::bail!("Invalid enum property value `{}`. Maybe you meant `{}`?", x, v);
                        }
                        valids += v;
                        valids += " ";
                    }
                    anyhow::bail!("Invalid enum property value `{}`. Valid values are: {}", x, valids);
                }
            },
            PVT::Numbery => Ok(PV::Numbery(x.parse()?)),
            PVT::Floaty => Ok(PV::Floaty(x.parse()?)),
            PVT::Booly => Ok(PV::Booly(x.parse()?)),
            PVT::SockAddr => Ok(PV::SockAddr(x.parse()?)),
            PVT::IpAddr => Ok(PV::IpAddr(x.parse()?)),
            PVT::PortNumber => Ok(PV::PortNumber(x.parse()?)),
            PVT::Path => todo!(),
            PVT::Uri => Ok(PV::Uri(x.parse()?)),
            PVT::Duration => todo!(),
            PVT::ChildNode => panic!(
                "You can't use PropertyValueType::interpret for obtaining child node pointers"
            ),
        }
    }
}

impl StrNode {
    #[tracing::instrument(name="StringyNode::build_impl", level="trace", skip(tree, registry, self, cli_opts), fields(node=&*self.name.0), err)]
    fn build_impl(
        &self,
        registry: &super::ClassRegistrar,
        cli_opts: &CliOpts,
        tree: &mut super::Slab<super::NodeId, super::DDataNode>,
    ) -> Result<super::NodeId> {
        tracing::debug!("Building parsed node");

        if let Some(cls) = registry.classes.get(&self.name.0) {
            use super::{PropertyValueType as PVT, PropertyValue as PV};
            use StringOrSubnode::{Str,Subnode};

            tracing::trace!("Obtained class: {:?}", cls);
            
            let mut b = cls.new_node();

            let props = cls.properties();

            if self.enable_autopopulate {
                for prop in &props {
                    if let Some(ref clip) = prop.inject_cli_long_option {
                        if let Some(vv) = cli_opts.get(clip) {
                            if vv.len() != 1 {
                                anyhow::bail!("Multiple instances of {} specified where only should should be", clip);
                            }
                            tracing::trace!("Setting property {} from CLI option {}", prop.name, clip);
                            b.set_property(&prop.name, vv[0].clone()).with_context(|| {
                                format!("Failed to set property {} in node {} from CLI options", prop.name, self.name.0)
                            })?;
                        }
                    }
                    if let Some(ref clip) = cls.array_inject_cli_long_opt() {
                        if let Some(vv) = cli_opts.get(clip) {
                            tracing::trace!("Setting the array from CLI option {}", clip);
                            for v in vv {
                                b.push_array_element(v.clone()).with_context(|| {
                                    format!("Failed to push array element to node {} from CLI options", self.name.0)
                                })?;
                            }
                        }
                    }
                }
            } else {
                tracing::trace!("Auto-population of CLI options is not enabled for this node");
            }

            let mut p: HashMap<String, super::PropertyValueType> =
                HashMap::with_capacity(props.len());
            p.extend(props.into_iter().map(|pi| (pi.name, pi.r#type)));


            for (Ident(k), v) in &self.properties {
                tracing::trace!("Handling property {}", k);
                if let Some(typ) = p.get(k) {
                    let vv = match (typ, v) {
                        (PVT::ChildNode, Subnode(x)) => PV::ChildNode(
                            x.build_impl(registry, cli_opts, tree).with_context(||format!(
                                "Building subbnode property {} value of node type {}",
                                k, self.name.0,
                            ))?,
                        ),
                        (_, Subnode(_)) => anyhow::bail!("A subnode is not expected as a property value {} of node {}", k, self.name.0),
                        (PVT::ChildNode, _) => anyhow::bail!("Subnode (`[...]`) expected as a property value {} of node {}", k, self.name.0),
                        (ty, Str(x)) => ty.interpret(x).with_context(|| {
                            format!(
                                "Failed to parse property {} in node {} that has value `{}`",
                                k, self.name.0, ValueForPrinting(x.clone())
                            )
                        })?,
                    };
                    b.set_property(k, vv).with_context(|| {
                        format!("Failed to set property {} in node {}", k, self.name.0)
                    })?;
                } else {
                    anyhow::bail!("Property {} of node type {} not found", k, self.name.0);
                }
            }

            let at = cls.array_type();

            for (n, e) in self.array.iter().enumerate() {
                tracing::trace!("Handling array element {}", n);
                if let Some(at) = &at {
                    let vv = match (at, e) {
                        (PVT::ChildNode, Subnode(x)) => PV::ChildNode(
                            x.build_impl(registry, cli_opts, tree).with_context(||format!(
                                "Building subnode array element number {} in node {}",
                                n, self.name.0,
                            ))?,
                        ),
                        (PVT::ChildNode, _) => anyhow::bail!("Subnode (`[...]`) expected as an array element number {} of node {}", n, self.name.0),
                        (ty, Str(x)) => ty.interpret(x).with_context(|| {
                            format!(
                                "Failed to array element number {} in node {} that has value `{}`",
                                n, self.name.0, ValueForPrinting(x.clone()),
                            )
                        })?,
                        (_, Subnode(_)) => anyhow::bail!("A subnode is not expected as an array element number {} of node {}", n, self.name.0),
                    };

                    b.push_array_element(vv)
                    .with_context(|| {
                        format!("Failed to push array element number `{}` to node {}", n, self.name.0)
                    })?;
                } else {
                    anyhow::bail!("Node type {} does not support array elements", self.name.0);
                }
            }

            tracing::trace!("Finished building");

            Ok(tree.insert(b.finish()?))
        } else {
            anyhow::bail!("Node type {} not found", self.name.0)
        }
    }

    /// Depth-first processing of all macros.
    /// Macros are eager to expand and should not see other macros nodes as input
    /// Upon expansion, the result is scanned for macros again
    #[tracing::instrument(name="process_macros", level="trace", skip(registry, self, cli_opts), fields(node=&*self.name.0), err)]
    fn process_macros(
        mut self,
        registry: &super::ClassRegistrar,
        cli_opts: &CliOpts,
    ) -> Result<StrNode> {

        for (_ident, content) in &mut self.properties {
            match content {
                StringOrSubnode::Str(_) => {}
                StringOrSubnode::Subnode(x) => {
                    let node = std::mem::take(x);
                    *x = node.process_macros(registry, cli_opts)?;
                }
            }
        }
        
        for content in &mut self.array {
            match content {
                StringOrSubnode::Str(_) => {}
                StringOrSubnode::Subnode(x) => {
                    let node = std::mem::take(x);
                    *x = node.process_macros(registry, cli_opts)?;
                }
            }
        }

        if let Some(r#macro) = registry.macros.get(&self.name.0) {
            tracing::trace!("Using macro {}", &self.name.0);

            let newstrnode = r#macro.run(self, cli_opts)?;
            newstrnode.process_macros(registry, cli_opts)
        } else {
            Ok(self)
        }
    }

    pub fn build(
        self,
        classes: &super::ClassRegistrar,
        cli_opts: &CliOpts,
        tree: &mut super::Tree,
    ) -> Result<super::NodeId> {
        self.process_macros(classes, cli_opts)?.build_impl(classes, cli_opts, tree)
    }

    /// Turn parsed node back into it's stringy representation

    #[tracing::instrument(name="StringyNode::reverse", level="trace", skip(tree), fields(), err)]
    pub fn reverse(root: super::NodeId, tree:&super::Tree) -> Result<Self> {
        tracing::debug!("Reversing a parsed node back to stringy representation");
        let n = tree.get(root).with_context(||"Node not found".to_string())?;
        let c = n.class();

        let name = Ident(c.official_name());
        let mut properties : Vec<(Ident, StringOrSubnode)> = Vec::new();
        let mut array : Vec<StringOrSubnode> = Vec::new();

        for super::PropertyInfo { name: pn, help: _, r#type, inject_cli_long_option: _ } in c.properties() {
            tracing::trace!("Processing property {}", pn);
            if let Some(v) = n.get_property(&pn) {
                tracing::trace!("Property {} is found", pn);
                let sn = match (v, r#type) {
                    (super::PropertyValue::ChildNode(q), super::PropertyValueType::ChildNode) => {
                        tracing::trace!("Descending into subnode {}", q.0);
                        StringOrSubnode::Subnode(StrNode::reverse(q, tree)?)
                    },
                    (_, super::PropertyValueType::ChildNode) => {
                        anyhow::bail!("Inconsistent property value for {} in node type {}", pn, name.0)
                    }
                    (super::PropertyValue::ChildNode(_), _) => {
                        anyhow::bail!("Inconsistent property value for {} in node type {}", pn, name.0)
                    }
                    (super::PropertyValue::Enummy(sym), super::PropertyValueType::Enummy(symtab)) => {
                        if let Some(s) = symtab.resolve(sym) {
                            let ss : Bytes = (s.to_owned()).into();
                            StringOrSubnode::Str(ss)
                        } else {
                            anyhow::bail!(
                                "Failed to resolve enum value {} for property {} in node type {}",
                                sym.to_usize(),
                                pn,
                                name.0,
                            )
                        }
                    }
                    (super::PropertyValue::Enummy(_), _) => {
                        anyhow::bail!("Inconsistent property value for {} in node type {}", pn, name.0)
                    }
                    (opv, _) => {
                        StringOrSubnode::Str(format!("{}", opv).into())
                    }
                };
                properties.push((Ident(pn), sn));
            }
        }

        for (n, el) in n.get_array().into_iter().enumerate() {
            tracing::trace!("Processing array element {}", n);
            let sn = match (el, c.array_type()) {
                (_, None) => {
                    anyhow::bail!("No array elements expected in node type {}", name.0)
                }
                (super::PropertyValue::ChildNode(q), Some(super::PropertyValueType::ChildNode)) => {
                    tracing::trace!("Descending into subnode {}", q.0);
                    StringOrSubnode::Subnode(StrNode::reverse(q, tree)?)
                },
                (_, Some(super::PropertyValueType::ChildNode)) => {
                    anyhow::bail!("Inconsistent array elment value in node type {}", name.0)
                }
                (super::PropertyValue::ChildNode(_), _) => {
                    anyhow::bail!("Inconsistent array element value in node type {}", name.0)
                }
                (opv, Some(_)) => {
                    StringOrSubnode::Str(format!("{}", opv).into())
                }
            };
            array.push(sn);
        }
        let enable_autopopulate = false;
        Ok(StrNode {
            name,
            properties,
            array,
            enable_autopopulate,
        })
    }
}

impl std::fmt::Display for super::PropertyValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            crate::PropertyValue::Stringy(x) => x.fmt(f),
            crate::PropertyValue::Enummy(sym) => {
                write!(f, "#{}", sym.to_usize())
            },
            crate::PropertyValue::Numbery(x) => x.fmt(f),
            crate::PropertyValue::Floaty(x) => x.fmt(f),
            crate::PropertyValue::Booly(x) => x.fmt(f),
            crate::PropertyValue::SockAddr(x) => x.fmt(f),
            crate::PropertyValue::IpAddr(x) => x.fmt(f),
            crate::PropertyValue::PortNumber(x) => x.fmt(f),
            crate::PropertyValue::Path(x) => match x.to_str() {
                Some(y) => y.fmt(f),
                None => "(?:/??)".fmt(f),
            },
            crate::PropertyValue::Uri(x) => x.fmt(f),
            crate::PropertyValue::Duration(_) => todo!(),
            crate::PropertyValue::ChildNode(_) => write!(f, "[???]"),
            crate::PropertyValue::BytesBuffer(x) => write!(f, "[len={}]", x.len()),
            crate::PropertyValue::OsString(x) => write!(f, "{:?}", x),
        }
    }
}
