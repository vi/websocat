use rhai::Engine;

use crate::scenario_executor::types::{
    DatagramRead, DatagramSocket, DatagramWrite, Hangup, StreamRead, StreamSocket, StreamWrite,
    Task, Handle,
};

pub fn register_functions(engine: &mut Engine) {
    crate::scenario_executor::trivials1::register(engine);
    crate::scenario_executor::trivials2::register(engine);
    crate::scenario_executor::copydata::register(engine);
    crate::scenario_executor::misc::register(engine);
    crate::scenario_executor::tcp::register(engine);
    crate::scenario_executor::fluff::register(engine);
    crate::scenario_executor::wsupgrade::register(engine);
    crate::scenario_executor::wsframer::register(engine);
    crate::scenario_executor::wswithpings::register(engine);
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
}
