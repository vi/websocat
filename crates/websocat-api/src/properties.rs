use super::*;

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
    /// Also used for command-line arguments array
    Path(PathBuf),

    /// Some URI or it's part
    Uri(http::Uri),

    /// Some interval of time
    Duration(Duration),

    /// Special string that originates from CLI arguments and may have values unrepresentable by `stringy::StrNode`.
    OsString(std::ffi::OsString),

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
    OsString,
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
            PropertyValueType::OsString   => write!(f,"osstring"),
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
            PropertyValueType::OsString   => PropertyValueTypeTag::OsString  ,
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
    OsString,
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
    
    /// Auto-add this option to CLI API. Specify the name without the leading `--`.
    /// Short options are privileged and cannot be auto-populated: there is explicit table of them in CLI crate. 
    pub inject_cli_long_option: Option<String>,
}

pub type Properties = HashMap<String, PropertyValue>;
