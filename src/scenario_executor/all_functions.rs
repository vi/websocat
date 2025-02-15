use std::net::SocketAddr;

use rhai::{Dynamic, Engine};

use super::http1::{
    Http1Client, IncomingRequest, IncomingResponse, OutgoingRequest, OutgoingResponse,
};
use super::types::{
    DatagramRead, DatagramSocket, DatagramWrite, Handle, Hangup, StreamRead, StreamSocket,
    StreamWrite, Task,
};
use std::ffi::OsString;
use tokio::process::{Child, Command};
use super::trivials3::{TriggerableEvent, TriggerableEventTrigger};

/// Register Rhai functions
pub fn register_functions(engine: &mut Engine) {
    super::trivials1::register(engine);
    super::trivials2::register(engine);
    super::trivials3::register(engine);
    super::linemode::register(engine);
    super::logoverlay::register(engine);
    super::copydata::register(engine);
    super::misc::register(engine);
    super::tcp::register(engine);
    super::udp::register(engine);
    super::udpserver::register(engine);
    super::fluff::register(engine);
    super::http1::register(engine);
    super::wsframer::register(engine);
    super::wswithpings::register(engine);
    #[cfg(feature="ssl")]
    super::nativetls::register(engine);
    super::subprocess::register(engine);
    super::osstr::register(engine);
    #[cfg(unix)]
    super::unix::register(engine);
    super::mockbytestream::register(engine);
    super::registryconnectors::register(engine);
    engine.register_fn("is_null", is_null);
}

#[macro_export]
macro_rules! all_types {
    ($x:ident) => {
        $x!(Task);
        $x!(Hangup);
        $x!(StreamRead);
        $x!(StreamWrite);
        $x!(StreamSocket);
        $x!(DatagramRead);
        $x!(DatagramWrite);
        $x!(DatagramSocket);
        $x!(SocketAddr);
        $x!(IncomingRequest);
        $x!(OutgoingResponse);
        $x!(OutgoingRequest);
        $x!(IncomingResponse);
        $x!(Http1Client);
        $x!(Command);
        $x!(Child);
        $x!(OsString);
        $x!(TriggerableEvent);
        $x!(TriggerableEventTrigger);
    };
}

/// Register most custom Rhai types.
pub fn register_types(engine: &mut Engine) {
    macro_rules! regtyp {
        ($t:ty) => {
            engine.register_type_with_name::<Handle<$t>>(stringify!($t));
        };
    }
    all_types!(regtyp);
}

//@ Check if given handle is null.
fn is_null(x: Dynamic) -> bool {
    macro_rules! check_for_type {
        ($t:ty) => {
            if let Some(x) = x.clone().try_cast::<Handle<$t>>() {
                return x.lock().unwrap().is_none();
            }
        };
    }
    crate::all_types!(check_for_type);

    false
}
