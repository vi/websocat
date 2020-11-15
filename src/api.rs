
//type Port = id_tree::NodeId;
//type Ports = Vec<Port>;//smallvec::SmallVec<[id_tree::NodeId; 1]>;

use std::collections::HashMap;
use std::collections::HashSet;
use tokio::prelude::{AsyncRead,AsyncWrite};
use std::future::Future;
use futures::stream::Stream;
use std::net::{SocketAddr,IpAddr};
use id_tree::{Tree,NodeId};
use downcast_rs::{impl_downcast,Downcast};
use std::fmt::Debug;
use std::sync::{Arc,Mutex};
use async_trait::async_trait;
use anyhow::Result;
use std::pin::Pin;

/// A part of parsed command line before looking up the SpecifierClasses.
pub struct StringyNode {
    pub name: String,
    pub properties: HashMap<String,String>,
    // pub child_nodes: id_tree::NodeId -- implied,
}
pub type StringyNodes = id_tree::Tree<StringyNode>;

/// Should I maybe somehow better used Serde model for this?
pub enum PropertyValue {
    /// A catch-all variant for properties lacking some dedicated thing
    Stringy(String),

    /// One of specific set of strings.
    /// 0 means the first value from PropertyValueType::Enummy vector (may be up to upper/lowercase)
    Enummy(usize),

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
}

pub enum PropertyValueType {
    Stringy,
    Enummy(Vec<String>),
    Numbery,
    Floaty,
    Booly,
    SockAddr,
    IpAddr,
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
    tree: Arc<Tree<DParsedNode>>,

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

/// Returned task is either spawned to dropped depending on settings.
/// Non-leaf (overlay) nodes should probably just pass this parameter down.
/// Leaf nodes that don't support accepting multiple connections should 
/// zero out (set to None) this parameter.
/// Leaf nodes that do support accepting multiple connections should 
/// interpret pre-exsting None as a signao to create and set up the socket for
/// serving connections and pre-existing Some as a signal to resume and accept
/// one more connection on existing socket (then leave another continuation
/// task in place of the one that is taken away)
type IWantToServeAnotherConnection = Option<Pin<Box<dyn Future<Output=()> + Send + 'static>>>;

/// Interpreted part of a command line describing some one aspect of a connection.
/// The tree of those is supposed to be checked and modified by linting engine.
/// Primary way to get those is by `SpecifierClass::parse`ing respective `StringyNode`s.
#[async_trait]
pub trait ParsedNode : Debug + Downcast{
    fn class(&self) -> DNodeClass;
    fn set_property(&mut self, name: String, val: PropertyValue) -> Result<()>;
    fn get_property(&self, name:String) -> Option<PropertyValue>;
    // not displayed here the fact that there are child nodes

    async fn run(&self, ctx: &RunContext, your_child_node_ids: Vec<NodeId>, multiconn: &mut IWantToServeAnotherConnection) -> Result<Pipe>;
}
impl_downcast!(ParsedNode);
pub type DParsedNode = Pin<Box<dyn ParsedNode + Send + 'static>>;


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

pub struct WebsocatContext {
    /// Place where specific nodes can store their process-global values
    /// Key should probably be `NodeClass::official_name`
    pub global_things: Arc<Mutex<Globals>>,

    pub left : Arc<Mutex<Tree<DParsedNode>>>,
    pub right : Arc<Mutex<Tree<DParsedNode>>>,

    /// Command-line options that do not belong to specific nodes
    pub global_parameters : Arc<Mutex<Properties>>,
}
/// Type of a connection type or filter or some other thing Websocat can use
pub trait NodeClass {
    /// Name of the class, like `tcp` or `ws`
    fn official_name(&self) -> String;
    /// List substrings that what can come before `:` to be considered belonging to this class.
    /// Should typically include `official_name()`. Like `["tcp-l:", "tcp-listen:", "listen-tcp:"]`.
    fn prefixes(&self) -> Vec<String>;
    /// Look up and apply all those properties, check that structure (i.e. number of children) is well-formed.
    /// Obviously, `class()` of the returned `DParsedNode` should lead back to this `SpecifierClass`. 
    fn parse(&self, x:&StringyNode, num_children: usize) -> Result<DParsedNode>;
    fn list_possible_properties(&self) -> Vec<PropertyInfo>;

    /// Return Err if linter detected error.
    /// Return non-empty vector if linter detected a warning
    /// Linter may rearrange or add notes, change properties, etc.
    ///
    /// Linter is expected to access WebsocatContext::left or ...::right based on `placement`, then look up the parsed node by `nodeid`,
    /// then downcast to to native node type, then check all the necessary things.
    fn run_lints(&self, nodeid: &NodeId, placement: NodePlacement, context: &WebsocatContext) -> Result<Vec<String>>;
}

pub type DNodeClass = Box<dyn NodeClass + Send + 'static>;

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