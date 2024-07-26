use std::net::SocketAddr;

use rhai::Engine;

use super::http1::{
    Http1Client, IncomingRequest, IncomingResponse, OutgoingRequest, OutgoingResponse,
};
use super::types::{
    DatagramRead, DatagramSocket, DatagramWrite, Handle, Hangup, StreamRead, StreamSocket,
    StreamWrite, Task,
};

pub fn register_functions(engine: &mut Engine) {
    super::trivials1::register(engine);
    super::trivials2::register(engine);
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
    super::nativetls::register(engine);
}

pub fn register_types(engine: &mut Engine) {
    macro_rules! regtyp {
        ($t:ty) => {
            engine.register_type_with_name::<Handle<$t>>(stringify!($t));
        };
    }
    regtyp!(Task);
    regtyp!(Hangup);
    regtyp!(StreamRead);
    regtyp!(StreamWrite);
    regtyp!(StreamSocket);
    regtyp!(DatagramRead);
    regtyp!(DatagramWrite);
    regtyp!(DatagramSocket);
    regtyp!(SocketAddr);
    regtyp!(IncomingRequest);
    regtyp!(OutgoingResponse);
    regtyp!(OutgoingRequest);
    regtyp!(IncomingResponse);
    regtyp!(Http1Client);
}
