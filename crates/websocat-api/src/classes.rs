use super::*;

pub trait NodeInProgressOfParsing {
    fn set_property(&mut self, name: &str, val: PropertyValue) -> Result<()>;
    fn push_array_element(&mut self, val: PropertyValue) -> Result<()>;
    
    fn finish(self: Box<Self>) -> Result<DDataNode>;
}
pub type DNodeInProgressOfParsing = Box<dyn NodeInProgressOfParsing + Send + 'static>;

#[derive(Copy,Clone,Debug,thiserror::Error)]
#[error("This node is a purely data storage and cannot be actually run")]
pub struct PurelyDataNodeError;

/// A storage for properties values.
/// Deriveable part of [`Node`].
/// Many of them can be converted to proper runnable [`Node`]s.
pub trait DataNode : Debug  {
    fn class(&self) -> DNodeClass;
    fn deep_clone(&self) -> DDataNode;

    fn get_property(&self, name:&str) -> Option<PropertyValue>;
    fn get_array(&self) -> Vec<PropertyValue>;

    /// Cast this Websocat node as runnable node, if possible
    fn upgrade(self: Pin<Arc<Self>>) -> ::std::result::Result<DRunnableNode, PurelyDataNodeError>;

    // Inherent method that is called after `NodeInProgressOfParsing::finish` if `validate` attribute is passed to the derive macro.
    // fn validate(&self) -> Result<()>;
}
pub type DDataNode = Pin<Arc<dyn DataNode + Send + Sync + 'static>>;

/// Interpreted part of a command line describing some one aspect of a connection.
/// The tree of those is supposed to be checked and modified by linting engine.
/// Primary way to get those is by `SpecifierClass::parse`ing respective `StringyNode`s.
#[async_trait]
pub trait RunnableNode : DataNode {
    /// Actually start the node (i.e. connect to TCP or recursively start another child node)
    /// If you want to serve multiple connections and `multiconn` is not None, you can
    /// trigger starting another Tokio task by using `multiconn`.
    async fn run(self: Pin<Arc<Self>>, ctx: RunContext, multiconn: Option<ServerModeContext>) -> Result<Bipipe>;
}
pub type DRunnableNode = Pin<Arc<dyn RunnableNode + Send + Sync + 'static>>;

pub struct NodeInATree<'a>(pub NodeId, pub &'a Tree);

pub enum NodePlacement {
    /// First positional argument of Websocat, "server side", connections acceptor
    Left,

    /// Second positional argument of Websocat
    Right,
}

pub type Tree = Slab<NodeId, DDataNode>;

#[derive(Clone)]
pub struct Circuit {
    pub nodes: Arc<Tree>,
    pub root : NodeId,
}

/// Set of auto-populated CLI options
pub type CliOpts = std::collections::HashMap<String, smallvec::SmallVec<[PropertyValue; 1]>>;

impl Circuit {
    /// Helper function, can be implemented using other low-level functions exposed by this crate
    pub fn build_from_tree_string(reg: &ClassRegistrar, cli_opts: &CliOpts,  x: &str) -> Result<Circuit> {
        let mut nodes = Tree::new();
    
        let q = StrNode::from_str(x).context("Parsing the tree")?;
        let root = q.build(&reg, cli_opts, &mut nodes).context("Building the tree")?;

        let c = Circuit::new(nodes, root);
        Ok(c)
    }


    /// Helper function, can be implemented using other low-level functions exposed by this crate
    pub fn build_from_tree_bytes(reg: &ClassRegistrar, cli_opts: &CliOpts,  x: &[u8]) -> Result<Circuit> {
        let mut nodes = Tree::new();
    
        let q = StrNode::from_bytes(x).context("Parsing the tree")?;
        let root = q.build(&reg, cli_opts, &mut nodes).context("Building the tree")?;

        let c = Circuit::new(nodes, root);
        Ok(c)
    }

    pub fn new_run_context(&self) -> RunContext {
        RunContext {
            nodes: self.nodes.clone(),
            left_to_right_things_to_be_filled_in: None,
            left_to_right_things_to_read_from: None,
        }
    }

    pub async fn run_root_node(&self) -> anyhow::Result<()> {
        let ctx = self.new_run_context();
        let dn = self.nodes[self.root].clone().upgrade()?;
        let ret = dn.run(ctx, None).await?;
        if ! matches!(ret.r, Source::None) {
            anyhow::bail!("Trying a node that returns a non-trivial source.")
        }
        if ! matches!(ret.w, Sink::None) {
            anyhow::bail!("Trying a node that returns a non-trivial sink")
        }
        Ok(())
    }

    #[must_use]
    pub fn new(nodes: Tree, root : NodeId) -> Circuit {
        Circuit {
            nodes: Arc::new(nodes),
            root,
        }
    }
}

/// Type of a connection type or filter or some other thing Websocat can use
pub trait NodeClass : Debug {
    /// Name of the class, like `tcp` or `ws`
    /// If name begins with a dot (`.`), it is considered soft-hidden
    fn official_name(&self) -> String;

    /// Obtain property names, their value types and documentation strings
    fn properties(&self) -> Vec<PropertyInfo>;

    /// Obtain type of node's associated array element type, if any
    fn array_type(&self) -> Option<PropertyValueType>;

    /// Obtain documentation string for the node's array, if any
    fn array_help(&self) -> Option<String>;

    /// Auto-add this option to CLI API for popularing array of nodes of this class.
    /// Specify the name without the leading `--`.
    fn array_inject_cli_long_opt(&self) -> Option<String>;

    /// Begin creating a new node
    fn new_node(&self) -> DNodeInProgressOfParsing;
    

    /// Return Err if linter detected error.
    /// Return non-empty vector if linter detected a warning
    /// Linter may rearrange or add notes, change properties, etc.
    ///
    /// Linter is expected to access `WebsocatContext::left` or `...::right` based on `placement`, then look up the parsed node by `nodeid`,
    /// then use `NodeProperyAccess` methods to check state of the nodes
    fn run_lints(&self, nodeid: NodeId, placement: NodePlacement, context: &Circuit) -> Result<Vec<String>>;
}

/// Typical propery name for child nodes
pub const INNER_NAME : &str = "inner";

pub type DNodeClass = Box<dyn NodeClass + Send + 'static>;

pub trait GetClassOfNode {
    type Class: NodeClass + Default + Send + 'static;
}

/// Converter of one StrNode to another
pub trait Macro {
    /// The name that should trigger the conversion
    fn official_name(&self) -> String;

    /// Register CLI long options for this macro 
    fn injected_cli_opts(&self) -> Vec<(String, CliOptionDescription)>;

    /// do the conversion
    fn run(&self, strnode: StrNode, cli_opts: &CliOpts) -> Result<StrNode>;
}
pub type DMacro = Box<dyn Macro + Send + 'static>;

#[derive(Default)]
// keys of hashmapes are officnames
pub struct ClassRegistrar {
    pub(crate) classes: HashMap<String, DNodeClass>,
    pub(crate) macros: HashMap<String, DMacro>,
}

#[derive(Clone,Eq,PartialEq)]
pub struct CliOptionDescription {
    pub typ: PropertyValueType,
    pub for_array: bool,
}

impl ClassRegistrar {
    pub fn register<N: GetClassOfNode>(&mut self) {
        self.register_impl(Box::new(N::Class::default()));
    }

    pub fn register_impl(&mut self, cls: DNodeClass) {
        let name = cls.official_name();
        if self.classes.contains_key(&name) {
            tracing::error!("Clashing websocat node classes for official name `{}`", name);
        }
        self.classes.insert(name, cls);
    }

    pub fn register_macro<M: Macro + Default + Send + 'static>(&mut self) {
        self.register_macro_impl(Box::new(M::default()))
    }

    pub fn register_macro_impl(&mut self, r#macro: DMacro) {
        let name = r#macro.official_name();
        if self.macros.contains_key(&name) {
            tracing::error!("Clashing websocat macro for official name `{}`", name);
        }
        self.macros.insert(name, r#macro);
    }

    pub fn classes(&self) -> impl Iterator<Item=&DNodeClass> {
        self.classes.iter().map(|(_,cls)|cls)
    }

    pub fn macros(&self) -> impl Iterator<Item=&DMacro> {
        self.macros.iter().map(|(_,cls)|cls)
    }
    pub fn classes_count(&self) -> usize { self.classes.len() }
    pub fn macros_count(&self) -> usize { self.macros.len() }
}

impl std::fmt::Debug for ClassRegistrar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.classes.keys().fmt(f)
    }
}
