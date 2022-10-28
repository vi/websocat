#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc)]

#[macro_use]
extern crate slab_typesafe;

use anyhow::Context;
pub use anyhow::Result;
use async_trait::async_trait;
use std::fmt::Debug;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{collections::HashMap, hash::Hash, str::FromStr};
use tokio::io::{AsyncRead, AsyncWrite};

pub mod stringy;
pub use stringy::StrNode;

pub extern crate anyhow;
pub extern crate async_trait;
pub extern crate bytes;
pub extern crate futures;
pub extern crate http;
pub extern crate smallvec;
pub extern crate string_interner;
pub extern crate tokio;
pub extern crate tracing;

declare_slab_token!(pub NodeId);

pub use slab_typesafe::Slab;

mod properties;

pub use properties::{
    Enum, EnummyTag, Properties, PropertyInfo, PropertyValue, PropertyValueType,
    PropertyValueTypeTag,
};

mod classes;

pub use classes::{
    Circuit, ClassRegistrar, CliOptionDescription, CliOpts, DDataNode, DMacro, DNodeClass,
    DNodeInProgressOfParsing, DRunnableNode, DataNode, GetClassOfNode, Macro, NodeClass,
    NodeInATree, NodeInProgressOfParsing, NodePlacement, PurelyDataNodeError, RunnableNode, Tree,
    INNER_NAME,
};

pub mod get_all_cli_options;

mod running;

pub use running::{
    AnyObject, Bipipe, ByteStreamSink, ByteStreamSource, ClosingNotification, DatagramSink,
    DatagramSource, HttpRequestWithResponseSlot, HttpSink, HttpSource, RunContext,
    ServerModeContext, Sink, Source,
};

#[cfg(feature = "sync")]
pub mod sync;

#[cfg(feature = "sync")]
pub use sync::Node as SyncNode;

/// Things not directly related to Websocat
pub mod util;
