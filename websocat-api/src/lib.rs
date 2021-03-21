#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc)]

#[macro_use]
extern crate slab_typesafe;


use std::{collections::HashMap, hash::Hash, str::FromStr};
use anyhow::Context;
use tokio::io::{AsyncRead,AsyncWrite};
use std::future::Future;
use std::net::{SocketAddr,IpAddr};
use std::path::PathBuf;
use downcast_rs::{impl_downcast,Downcast};
use std::fmt::Debug;
use std::sync::{Arc,Mutex};
use async_trait::async_trait;
pub use anyhow::Result;
use std::pin::Pin;
use std::time::Duration;

pub mod stringy;
pub use stringy::StrNode;

pub extern crate anyhow;
pub extern crate tokio;
pub extern crate async_trait;
pub extern crate string_interner;
pub extern crate bytes;
pub extern crate futures;

declare_slab_token!(pub NodeId);

pub use slab_typesafe::Slab;

/// On of the value of an enum-based property
pub struct EnummyTag(pub usize);

/// Should I maybe somehow better used Serde model for this?
#[derive(Debug,Clone)]
pub enum PropertyValue {
    /// A catch-all variant for properties lacking some dedicated thing
    Stringy(String),

    /// Some block of bytes
    BytesBuffer(bytes::Bytes),

    /// One of specific set of strings.
    /// 0 means the first value from PropertyValueType::Enummy vector (may be up to upper/lowercase)
    Enummy(string_interner::DefaultSymbol),

    /// Numberic
    Numbery(i64),

    /// Something fractional
    Floaty(f64),

    /// A boolean-valued property
    Booly(bool),

    /// Some IPv4 or IPv6 address with a port number
    SockAddr(SocketAddr),

    /// Some IPv4 or IPv6 address
    IpAddr(IpAddr),

    /// A port number
    PortNumber(u16),

    /// Some file or directory name
    Path(PathBuf),

    /// Some URI or it's part
    Uri(http::Uri),

    /// Some interval of time
    Duration(Duration),

    /// Some source and sink of byte blocks
    ChildNode(NodeId),
}

#[derive(Debug,Clone,Eq,PartialEq)]
pub enum PropertyValueType {
    Stringy,
    BytesBuffer,
    Enummy(string_interner::StringInterner),
    Numbery,
    Floaty,
    Booly,
    SockAddr,
    IpAddr,
    PortNumber,
    Path,
    Uri,
    Duration,
    ChildNode,

    // pub fn interpret(&self, x: &str) -> Result<PropertyValue>;
}

impl std::fmt::Display for PropertyValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropertyValueType::Enummy(x)  => {
                write!(f, "enum(")?;
                for (n,(_,v)) in x.into_iter().enumerate() {
                    if n > 0 { write!(f, ",")?; }
                    write!(f, "{}", v)?;
                }
                write!(f, ")")?;
                Ok(())
            }
            PropertyValueType::Stringy    => write!(f,"string"),
            PropertyValueType::BytesBuffer => write!(f,"bytes"),
            PropertyValueType::Numbery    => write!(f,"number"),
            PropertyValueType::Floaty     => write!(f,"float"),
            PropertyValueType::Booly      => write!(f,"bool"),
            PropertyValueType::SockAddr   => write!(f,"sockaddr"),
            PropertyValueType::IpAddr     => write!(f,"ipaddr"),
            PropertyValueType::PortNumber => write!(f,"portnumber"),
            PropertyValueType::Path       => write!(f,"path"),
            PropertyValueType::Uri        => write!(f,"uri"),
            PropertyValueType::Duration   => write!(f,"duration"),
            PropertyValueType::ChildNode  => write!(f,"subnode"),
        }
    }
}

impl PropertyValueType {
    pub fn tag(&self) -> PropertyValueTypeTag {
        match self {
            PropertyValueType::Stringy    => PropertyValueTypeTag::Stringy,
            PropertyValueType::BytesBuffer=> PropertyValueTypeTag::BytesBuffer,
            PropertyValueType::Enummy(_)  => PropertyValueTypeTag::Enummy,
            PropertyValueType::Numbery    => PropertyValueTypeTag::Numbery,
            PropertyValueType::Floaty     => PropertyValueTypeTag::Floaty    ,
            PropertyValueType::Booly      => PropertyValueTypeTag::Booly     ,
            PropertyValueType::SockAddr   => PropertyValueTypeTag::SockAddr  ,
            PropertyValueType::IpAddr     => PropertyValueTypeTag::IpAddr    ,
            PropertyValueType::PortNumber => PropertyValueTypeTag::PortNumber,
            PropertyValueType::Path       => PropertyValueTypeTag::Path      ,
            PropertyValueType::Uri        => PropertyValueTypeTag::Uri       ,
            PropertyValueType::Duration   => PropertyValueTypeTag::Duration  ,
            PropertyValueType::ChildNode  => PropertyValueTypeTag::ChildNode ,
        }
    }
}

#[derive(Debug,Clone,Eq,PartialEq,Ord,PartialOrd,Hash)]
pub enum PropertyValueTypeTag {
    Stringy,
    BytesBuffer,
    Enummy,
    Numbery,
    Floaty,
    Booly,
    SockAddr,
    IpAddr,
    PortNumber,
    Path,
    Uri,
    Duration,
    ChildNode,
}

/// Deriveable information for making Enummy based on usual Rust enums
pub trait Enum {
    /// Construct interner that stores (maybe lowercase) enum variant names, with predictable symbol numeric values
    fn interner() -> string_interner::StringInterner;

    /// Construct variant by index
    fn index_to_variant(sym: string_interner::DefaultSymbol) -> Self;

    /// Get index of a variant
    fn variant_to_index(&self) -> string_interner::DefaultSymbol;
}

/// A user-facing information block about some property of some `NodeClass`
pub struct PropertyInfo {
    pub name: String,
    pub help: Box<dyn Fn()->String + Send + 'static>,
    pub r#type: PropertyValueType,
    
    /// Auto-add this option to CLI API. Specify the name without trailing `--`.
    /// Short options are privileged and cannot be auto-populated: there is explicit table of them in CLI crate. 
    pub inject_cli_long_option: Option<String>,
}

type Properties = HashMap<String, PropertyValue>;

#[derive(Clone)]
pub struct RunContext {
    /// for starting running child nodes before this one
    pub nodes: Arc<Tree>,

    /// Mutually exclusive with `left_to_right_things_to_read_from`
    /// Used "on the left (server) sise" of websocat call to fill in various
    /// incoming connection parameters like IP address or requesting URL. 
    ///
    /// Hashmap keys are arbitrary identifiers - various nodes need to aggree in them
    pub left_to_right_things_to_be_filled_in: Option<Arc<Mutex<Properties>>>,

    /// Mutually exclusive with `left_to_right_things_to_be_filled_in`
    /// Use d "on the right side" of websocat call to act based on properties
    /// collected during acceping incoming connection
    pub left_to_right_things_to_read_from: Option<Arc<Mutex<Properties>>>,

    pub globals: Arc<Mutex<Globals>>,
}

static_assertions::assert_impl_all!(RunContext : Send);

/// Opaque object that can be used as a storage space for individual nodes
pub type AnyObject = Box<dyn std::any::Any + Send + 'static>;

/// Used to support serving multiple clients, allowing to restart Websocat session from
/// nodes like "tcp-listen", passing listening sockets though `AnyObject`.
/// 
/// First time `you_are_called_not_the_first_time` is None, meaning that e.g. `TcpListener` should be
/// created from scratch.
/// 
/// Invoking `call_me_again_with_this` spawns a Tokio task that should ultimately return back
/// to the node that issued `call_me_again_with_this`, but with `you_are_called_not_the_first_time`
/// filled in, so `TcpListener` (with potential next pending connection) should be restored
/// from the `AnyObject` instead of being created from stratch. 
pub struct ServerModeContext {
    pub you_are_called_not_the_first_time: Option<AnyObject>,

    #[allow(clippy::unused_unit)]
    pub call_me_again_with_this: Box<dyn FnOnce(AnyObject) -> () + Send + 'static>,
}

pub trait NodeInProgressOfParsing {
    fn set_property(&mut self, name: &str, val: PropertyValue) -> Result<()>;
    fn push_array_element(&mut self, val: PropertyValue) -> Result<()>;
    
    fn finish(self: Box<Self>) -> Result<DNode>;
}
pub type DNodeInProgressOfParsing = Box<dyn NodeInProgressOfParsing + Send + 'static>;

/// Deriveable part of [`Node`].
pub trait NodeProperyAccess : Debug  {
    fn class(&self) -> DNodeClass;
    fn deep_clone(&self) -> DNode;

    fn get_property(&self, name:&str) -> Option<PropertyValue>;
    fn get_array(&self) -> Vec<PropertyValue>;

    // Inherent method that is called after `NodeInProgressOfParsing::finish` if `validate` attribute is passed to the derive macro.
    // fn validate(&self) -> Result<()>;
}

/// Interpreted part of a command line describing some one aspect of a connection.
/// The tree of those is supposed to be checked and modified by linting engine.
/// Primary way to get those is by `SpecifierClass::parse`ing respective `StringyNode`s.
#[async_trait]
pub trait Node : NodeProperyAccess + Downcast {
    /// Actually start the node (i.e. connect to TCP or recursively start another child node)
    /// If you want to serve multiple connections and `multiconn` is not None, you can
    /// trigger starting another Tokio task by using `multiconn`.
    async fn run(self: Pin<Arc<Self>>, ctx: RunContext, multiconn: Option<ServerModeContext>) -> Result<Bipipe>;
}
impl_downcast!(Node);
pub type DNode = Pin<Arc<dyn Node + Send + Sync + 'static>>;

pub struct NodeInATree<'a>(pub NodeId, pub &'a Tree);


pub trait GlobalInfo : Debug + Downcast {

}
impl_downcast!(GlobalInfo);
type Globals = HashMap<String, Box<dyn GlobalInfo + Send + 'static>>;

pub enum NodePlacement {
    /// First positional argument of Websocat, "server side", connections acceptor
    Left,

    /// Second positional argument of Websocat
    Right,
}

pub type Tree = Slab<NodeId, DNode>;

#[derive(Clone)]
pub struct Session {
    /// Place where specific nodes can store their process-global values
    /// Key should probably be `NodeClass::official_name`
    pub global_things: Arc<Mutex<Globals>>,

    pub nodes: Arc<Tree>,

    pub left : NodeId,
    pub right : NodeId,

    /// Command-line options that do not belong to specific nodes
    pub global_parameters : Arc<Mutex<Properties>>,
}

impl Session {
    /// Helper function, can be implemented using other low-level functions exposed by this crate
    pub fn build_from_two_tree_strings(reg: &ClassRegistrar, cli_opts: &std::collections::HashMap<String,PropertyValue>,  left: &str, right: &str) -> Result<Session> {
        let mut t = Tree::new();
    
        let q = StrNode::from_str(left).context("Parsing the left tree")?;
        let w = q.build(&reg, cli_opts, &mut t).context("Building the left tree")?;
    
        let q2 = StrNode::from_str(right).context("Parsing the right tree")?;
        let w2 = q2.build(&reg, cli_opts, &mut t).context("Building the right tree")?;

        let c = Session::new(t, w, w2);
        Ok(c)
    }
}

impl Session {
    #[must_use]
    pub fn new(nodes: Tree, left : NodeId, right : NodeId) -> Session {
        Session {
            nodes: Arc::new(nodes),
            left,
            right,
            global_parameters: Arc::new(Mutex::new(Properties::new())),
            global_things: Arc::new(Mutex::new(Globals::new())),
        }
    }
}

/// Type of a connection type or filter or some other thing Websocat can use
pub trait NodeClass : Debug {
    /// Name of the class, like `tcp` or `ws`
    fn official_name(&self) -> String;
    /// List substrings that what can come before `:` to be considered belonging to this class.
    /// Should typically include `official_name()`. Like `["tcp-l:", "tcp-listen:", "listen-tcp:"]`.
    /// Also used for matching the `StringyNode::name`s to the node classes.
    fn prefixes(&self) -> Vec<String>;

    /// Obtain property names, their value types and documentation strings
    fn properties(&self) -> Vec<PropertyInfo>;

    /// Obtain type of node's associated array element type, if any
    fn array_type(&self) -> Option<PropertyValueType>;

    /// Obtain documentation string for the node's array, if any
    fn array_help(&self) -> Option<String>;

    /// Begin creating a new node
    fn new_node(&self) -> DNodeInProgressOfParsing;
    

    /// Return Err if linter detected error.
    /// Return non-empty vector if linter detected a warning
    /// Linter may rearrange or add notes, change properties, etc.
    ///
    /// Linter is expected to access `WebsocatContext::left` or `...::right` based on `placement`, then look up the parsed node by `nodeid`,
    /// then downcast to to native node type, then check all the necessary things.
    fn run_lints(&self, nodeid: NodeId, placement: NodePlacement, context: &Session) -> Result<Vec<String>>;
}

/// Typical propery name for child nodes
pub const INNER_NAME : &str = "inner";

pub type DNodeClass = Box<dyn NodeClass + Send + 'static>;

pub trait GetClassOfNode {
    type Class: NodeClass + Default + Send + 'static;
}

#[derive(Default)]
pub struct ClassRegistrar {
    pub(crate) officname_to_classes: HashMap<String, DNodeClass>,
}

impl ClassRegistrar {
    pub fn register<N: GetClassOfNode>(&mut self) {
        self.register_impl(Box::new(N::Class::default()));
    }

    pub fn register_impl(&mut self, cls: DNodeClass) {
        let name = cls.official_name();
        if self.officname_to_classes.contains_key(&name) {
            tracing::error!("Clashing websocat node classes for official name `{}`", name);
        }
        self.officname_to_classes.insert(name, cls);
    }

    /// Get all class-injected long CLI options with their types
    pub fn get_all_cli_options(&self) -> Result<HashMap<String, PropertyValueType>> {
        let mut v = HashMap::with_capacity(32);
        // for error reporintg
        let mut provenance = <HashMap<String,String>>::with_capacity(32);

        for k in self.officname_to_classes.values() {
            for ref p in k.properties() {
                if let Some(ref clin) = p.inject_cli_long_option {
                    let prov = format!("{}::{}",  k.official_name(), p.name);
                    if p.r#type.tag() == PropertyValueTypeTag::ChildNode {
                        anyhow::bail!(
                            "Internal error: attempt to create CLI option `{}` that maps to subnode-typed property `{}`.",
                            clin,
                            prov,
                        );
                    }
                    match v.entry(clin.clone()) {
                        std::collections::hash_map::Entry::Occupied(x) => {
                            if x.get() == &p.r#type {
                                tracing::debug!(
                                    "CLI long option `{}` of type `{}` also maps to `{}`",
                                    clin,
                                    p.r#type,
                                    prov,
                                );
                            } else {
                                anyhow::bail!(
                                    "Internal error: conflicting types of long CLI option `{}`. Accorting to `{}` it should be `{}`, but according to `{}` it should be `{}`.",
                                    clin,
                                    provenance[&**clin],
                                    x.get(),
                                    prov,
                                    p.r#type,
                                );
                            }
                        }
                        std::collections::hash_map::Entry::Vacant(x) => {
                            tracing::debug!(
                                "Inserting global CLI long option: `{}` of type `{}`, mapping to `{}`",
                                clin,
                                p.r#type,
                                prov,
                            );
                            provenance.insert(clin.clone(), prov);
                            x.insert(p.r#type.clone());
                        }
                    }
                }
            }
        }

        Ok(v)
    }
}

impl std::fmt::Debug for ClassRegistrar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.officname_to_classes.keys().fmt(f)
    }
}

pub enum Source {
    ByteStream(Pin<Box<dyn AsyncRead + Send  + 'static>>),
    Datagrams(Pin<Box<dyn futures::stream::Stream<Item=Result<bytes::Bytes>> + Send  + 'static>>),
    None,
}


pub enum Sink {
    ByteStream(Pin<Box<dyn AsyncWrite + Send  + 'static>>),
    Datagrams(Pin<Box<dyn futures::sink::Sink<bytes::Bytes, Error=anyhow::Error> + Send  + 'static>>),
    None,
}

/// A bi-directional channel + special closing notification
pub struct Bipipe {
    pub r: Source,
    pub w: Sink,
    pub closing_notification: Option<Pin<Box<dyn Future<Output=()> + Send + 'static>>>,
}

impl std::fmt::Debug for Bipipe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.r {
            Source::ByteStream(..) => write!(f, "(r=ByteStream")?,
            Source::Datagrams(..) =>  write!(f, "(r=Datagrams")?,
            Source::None =>  write!(f, "(r=None")?,
        };
        match self.w {
            Sink::ByteStream(..) => write!(f, " w=ByteStream")?,
            Sink::Datagrams(..) =>  write!(f, " w=Datagrams")?,
            Sink::None =>  write!(f, " w=None")?,
        };
        if self.closing_notification.is_some() {
            write!(f, " +CN)")?;
        } else {
            write!(f, ")")?;
        }
        Ok(())
    }
}


#[cfg(feature="sync")]
pub mod sync;


#[cfg(feature="sync")]
pub use sync::Node as SyncNode;

/// Things not directly related to Websocat
pub mod util;
