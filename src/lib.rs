pub mod scenario_executor {
    pub mod copydata;

    pub mod debugfluff;
    pub mod fluff;
    pub mod http1;
    pub mod lengthprefixed;
    pub mod linemode;
    pub mod logoverlay;
    pub mod misc;
    pub mod mockbytestream;
    #[cfg(feature = "ssl")]
    pub mod nativetls;
    #[cfg(feature = "rustls")]
    pub mod rustls;
    pub mod osstr;
    pub mod registryconnectors;
    pub mod reuser;
    pub mod scenario;
    pub mod subprocess;
    pub mod tcp;
    pub mod trivials1;
    pub mod trivials2;
    pub mod trivials3;
    pub mod file;
    pub mod types;
    pub mod udp;
    pub mod udpserver;
    #[cfg(unix)]
    pub mod unix1;
    #[cfg(unix)]
    pub mod unix2;
    pub mod utils1;
    pub mod utils2;
    pub mod wsframer;
    pub mod wswithpings;

    pub mod all_functions;

    pub const MAX_CONTROL_MESSAGE_LEN: usize = 65536;
}

pub mod scenario_planner {
    pub mod buildscenario;
    pub mod buildscenario_endpoints;
    pub mod buildscenario_exec;
    pub mod buildscenario_misc;
    pub mod buildscenario_overlays;
    pub mod buildscenario_tcp;
    pub mod buildscenario_udp;
    pub mod buildscenario_unix;
    pub mod buildscenario_ws;
    pub mod fromstr;
    pub mod linter;
    pub mod patcher;
    pub mod scenarioprinter;
    pub mod types;
    pub mod utils;
}

pub mod cli;
pub mod composed_cli;
pub mod test_utils;
pub mod base;

pub use base::websocat_main;
