use std::net::SocketAddr;

use rhai::Engine;

use super::types::{
    DatagramRead, DatagramSocket, DatagramWrite, Handle, Hangup, StreamRead, StreamSocket,
    StreamWrite, Task,
};
use super::wsupgrade::{IncomingRequest,OutgoingResponse};

pub fn register_functions(engine: &mut Engine) {
    super::trivials1::register(engine);
    super::trivials2::register(engine);
    super::copydata::register(engine);
    super::misc::register(engine);
    super::tcp::register(engine);
    super::fluff::register(engine);
    super::wsupgrade::register(engine);
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
}
