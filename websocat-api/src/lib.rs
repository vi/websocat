#[macro_use]
extern crate slab_typesafe;

use std::collections::HashMap;
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
pub use stringy::StringyNode;

pub extern crate anyhow;
pub extern crate tokio;
pub extern crate async_trait;

declare_slab_token!(pub NodeId);

pub use slab_typesafe::Slab;

/// On of the value of an enum-based property
pub struct EnummyTag(pub usize);

/// Should I maybe somehow better used Serde model for this?
pub enum PropertyValue {
    /// A catch-all variant for properties lacking some dedicated thing
    Stringy(String),

    /// One of specific set of strings.
    /// 0 means the first value from PropertyValueType::Enummy vector (may be up to upper/lowercase)
    Enummy(EnummyTag),

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

#[derive(Debug,Clone,Eq,PartialEq,Ord,PartialOrd)]
pub enum PropertyValueType {
    Stringy,
    Enummy(Vec<String>),
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

/// A user-facing information block about some property of some SpecifierClass
pub struct PropertyInfo {
    pub name: String,
    pub help: String,
    pub r#type: PropertyValueType,
}

type Properties = HashMap<String, PropertyValue>;

#[derive(Clone)]
pub struct RunContext {
    /// for starting running child nodes before this one
    tree: Arc<Tree>,

    /// Mutually exclusive with `left_to_right_things_to_read_from`
    /// Used "on the left (server) sise" of websocat call to fill in various
    /// incoming connection parameters like IP address or requesting URL. 
    ///
    /// Hashmap keys are arbitrary identifiers - various nodes need to aggree in them
    left_to_right_things_to_be_filled_in: Option<Arc<Mutex<Properties>>>,

    /// Mutually exclusive with `left_to_right_things_to_be_filled_in`
    /// Use d "on the right side" of websocat call to act based on properties
    /// collected during acceping incoming connection
    left_to_right_things_to_read_from: Option<Arc<Mutex<Properties>>>,

    globals: Arc<Mutex<Globals>>,
}

static_assertions::assert_impl_all!(RunContext : Send);

/// Returned task is either spawned to dropped depending on settings.
/// Non-leaf (overlay) nodes should probably just pass this parameter down.
/// Leaf nodes that don't support accepting multiple connections should 
/// zero out (set to None) this parameter.
/// Leaf nodes that do support accepting multiple connections should 
/// interpret pre-exsting None as a signao to create and set up the socket for
/// serving connections and pre-existing Some as a signal to resume and accept
/// one more connection on existing socket (then leave another continuation
/// task in place of the one that is taken away)
pub type IWantToServeAnotherConnection = Option<Pin<Box<dyn Future<Output=()> + Send + 'static>>>;

pub trait NodeInProgressOfParsing {
    fn set_property(&mut self, name: &str, val: PropertyValue) -> Result<()>;
    fn push_array_element(&mut self, val: PropertyValue) -> Result<()>;
    
    fn finish(self: Box<Self>) -> Result<DParsedNode>;
}
pub type DNodeInProgressOfParsing = Box<dyn NodeInProgressOfParsing + Send + 'static>;

/// Deriveable part of ParsedNode.
pub trait ParsedNodeProperyAccess : Debug  {
    fn class(&self) -> DNodeClass;
    fn clone(&self) -> DParsedNode;

    fn get_property(&self, name:&str) -> Option<PropertyValue>;
    fn get_array(&self) -> Vec<PropertyValue>;

    // not displayed here the fact that there are child nodes
}

/// Interpreted part of a command line describing some one aspect of a connection.
/// The tree of those is supposed to be checked and modified by linting engine.
/// Primary way to get those is by `SpecifierClass::parse`ing respective `StringyNode`s.
#[async_trait]
pub trait ParsedNode : ParsedNodeProperyAccess + Downcast {
    async fn run(&self, ctx: RunContext, multiconn: &mut IWantToServeAnotherConnection) -> Result<Pipe>;
}
impl_downcast!(ParsedNode);
pub type DParsedNode = Pin<Box<dyn ParsedNode + Send + Sync + 'static>>;

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

pub type Tree = Slab<NodeId, DParsedNode>;

pub struct WebsocatContext {
    /// Place where specific nodes can store their process-global values
    /// Key should probably be `NodeClass::official_name`
    pub global_things: Arc<Mutex<Globals>>,

    pub nodes: Arc<Tree>,

    pub left : NodeId,
    pub right : NodeId,

    /// Command-line options that do not belong to specific nodes
    pub global_parameters : Arc<Mutex<Properties>>,
}
/// Type of a connection type or filter or some other thing Websocat can use
pub trait NodeClass {
    /// Name of the class, like `tcp` or `ws`
    fn official_name(&self) -> String;
    /// List substrings that what can come before `:` to be considered belonging to this class.
    /// Should typically include `official_name()`. Like `["tcp-l:", "tcp-listen:", "listen-tcp:"]`.
    /// Also used for matching the `StringyNode::name`s to the node classes.
    fn prefixes(&self) -> Vec<String>;

    fn properties(&self) -> Vec<PropertyInfo>;
    fn array_type(&self) -> Option<PropertyValueType>;

    fn new_node(&self) -> DNodeInProgressOfParsing;
    

    /// Return Err if linter detected error.
    /// Return non-empty vector if linter detected a warning
    /// Linter may rearrange or add notes, change properties, etc.
    ///
    /// Linter is expected to access WebsocatContext::left or ...::right based on `placement`, then look up the parsed node by `nodeid`,
    /// then downcast to to native node type, then check all the necessary things.
    fn run_lints(&self, nodeid: &NodeId, placement: NodePlacement, context: &WebsocatContext) -> Result<Vec<String>>;
}

/// Typical propery name for child nodes
pub const INNER_NAME : &'static str = "inner";

pub type DNodeClass = Box<dyn NodeClass + Send + 'static>;

pub trait NodeType {
    type Class: NodeClass;
}


/*
type PendingPipe  = Box<dyn FnOnce() -> Box<dyn Future<Output=anyhow::Result<Pipe>> + Send + Sync + 'static> + Send + Sync + 'static>;
type PendingPipes = Box<dyn FnOnce() -> Box<dyn Stream<Item=anyhow::Result<Pipe>> + Send + Sync + 'static> + Send + Sync + 'static>;
type PendingOverlay = Box<dyn FnMut(Vec<Pipe>) ->  Box<dyn Future<Output=anyhow::Result<Pipe>> + Send + Sync + 'static> + Send + Sync + 'static>;

pub enum ArmedNode {
    ReadyToOverlay(PendingOverlay),
    ReadyToProduceAConnection(PendingPipe),
    ReadyToProduceMultipleConnections(PendingPipes),
}
*/


/// A bi-directional channel + special closing notification
/// Message boundaries in AsyncRead / AsyncWrite poll calls may (or may not) matter - it depends on contenxt 
pub struct Pipe {
    pub r: Pin<Box<dyn AsyncRead + Send  + 'static>>,
    pub w: Pin<Box<dyn AsyncWrite + Send  + 'static>>,
    pub closing_notification: Option<Pin<Box<dyn Future<Output=()> + Send + 'static>>>,
}
//type PendingPipe = Box<dyn Future<Output=Result<Pipe>> + Send + Sync + 'static>;


// /// State where all intermediate representations are "frozen" into a thing that can actually be "launched" by Tokio
// type ReadyToGo = Box<dyn Future<Output=()> + Send + Sync + 'static>;