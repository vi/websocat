pub use super::proxy_peer::SocksSocketAddr;

use super::readdebt::DebtHandling;

#[derive(Debug, Clone)]
pub struct StaticFile {
    pub uri: String,
    pub file: ::std::path::PathBuf,
    pub content_type: String,
}

#[derive(SmartDefault, Debug, Clone)]
pub struct Options {
    pub websocket_text_mode: bool,
    pub websocket_protocol: Option<String>,
    pub udp_oneshot_mode: bool,
    pub unidirectional: bool,
    pub unidirectional_reverse: bool,
    pub exit_on_eof: bool,
    pub oneshot: bool,
    pub unlink_unix_socket: bool,
    pub exec_args: Vec<String>,
    pub ws_c_uri: String,
    pub linemode_strip_newlines: bool,
    pub linemode_strict: bool,
    pub origin: Option<String>,
    pub custom_headers: Vec<(String, Vec<u8>)>,
    pub websocket_version: Option<String>,
    pub websocket_dont_close: bool,
    pub one_message: bool,
    pub no_auto_linemode: bool,
    #[default = "65536"]
    pub buffer_size: usize,
    #[default = "16"]
    pub broadcast_queue_len: usize,
    #[default = "DebtHandling::Silent"]
    pub read_debt_handling: DebtHandling,
    pub linemode_zero_terminated: bool,
    pub restrict_uri: Option<String>,
    pub serve_static_files: Vec<StaticFile>,
    pub exec_set_env: bool,
    pub reuser_send_zero_msg_on_disconnect: bool,
    pub process_zero_sighup: bool,
    pub process_exit_sighup: bool,
    pub socks_destination: Option<SocksSocketAddr>,
}
