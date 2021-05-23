#![allow(unused)]

use std::str::FromStr;

use websocat_api::{ServerModeContext, string_interner::Symbol};

#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name = "foo",
    prefix="foo",
    prefix="f",
)]
struct Foo {
    /// OOO
    o : i64,

    /// lol
    inner : websocat_api::NodeId,

    /// Whatever,
    ///  a multi-line
    /// docstr"tring``g.
    t : Option<String>,
}


#[websocat_api::async_trait::async_trait]
impl websocat_api::RunnableNode for Foo {
    async fn run(self: std::pin::Pin<std::sync::Arc<Self>>, ctx: websocat_api::RunContext, _multiconn: Option<ServerModeContext>) -> websocat_api::Result<websocat_api::Bipipe> {
        Err(websocat_api::anyhow::anyhow!("nimpl"))
    }
}


#[derive(Debug, Clone,Copy,websocat_derive::WebsocatEnum)]
#[websocat_enum(
    rename_all_lowercase,
    //debug_derive,
)]
enum Qqq {
    Hoo,
    Loo,
    Aoo,
    #[websocat_enum(rename = "jjj2")]
    Coo,
    Phh,
}


#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name = "bar",
    //debug_derive,
)]
struct Bar {
    /// Content to print
    #[websocat_prop(enum)]
    content : Vec<Qqq>,
}

impl websocat_api::sync::Node for Bar {
    fn run(self: std::pin::Pin<std::sync::Arc<Self>>, ctx: websocat_api::RunContext, allow_multiconnect: bool, mut closure: impl FnMut(websocat_api::sync::Bipipe) -> websocat_api::Result<()> + Send + 'static ) -> websocat_api::Result<()> {
        let (r,mut w2) = pipe::pipe();
        let w = std::io::sink();
        std::thread::spawn(move|| {
            closure(websocat_api::sync::Bipipe {
                r: websocat_api::sync::Source::ByteStream(Box::new(r)),
                w: websocat_api::sync::Sink::ByteStream(Box::new(w)),
                closing_notification: None,
            });
        });
        let this = self.clone();
        std::thread::spawn(move|| {
            for _ in 0..10 {
                use std::io::Write;
                let s = format!("{:?}\n", this.content);
                w2.write_all(s.as_bytes()).unwrap();
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        });
        Ok(())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt::init();

    let mut reg = websocat_api::ClassRegistrar::default();
    reg.register::<Foo>();
    reg.register::<Bar>();
    reg.register::<websocat_basic::net::Tcp>();
    reg.register::<websocat_basic::net::TcpListen>();
    reg.register::<websocat_basic::io_std::Stdio>();
    reg.register::<websocat_syncnodes::net::TcpConnect>();
    reg.register::<websocat_syncnodes::net::TcpListen>();
    reg.register::<websocat_syncnodes::net::UdpConnect>();
    reg.register::<websocat_syncnodes::net::UdpListen>();
    reg.register::<websocat_http::HttpClient>();

    //println!("{:?}", reg);

    let args = std::env::args().collect::<Vec<_>>();
    let cliopts = std::collections::HashMap::new();

    let c = websocat_api::Session::build_from_two_tree_strings(
        &reg, 
        &cliopts,
        &args[1],
        &args[2],
    ).unwrap();

    println!("{}", websocat_api::StrNode::reverse(c.left, &c.nodes).unwrap());
    println!("{}", websocat_api::StrNode::reverse(c.right, &c.nodes).unwrap());
    

    if let Err(e) = websocat_session::run(websocat_session::Opts{enable_backward: false, enable_forward: true}, c).await {
        eprintln!("Error: {:#}", e);
    }
}
