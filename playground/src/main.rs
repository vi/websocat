#![allow(unused)]

use std::str::FromStr;

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
impl websocat_api::Node for Foo {
    async fn run(&self, ctx: websocat_api::RunContext, _multiconn: Option<&mut websocat_api::IWantToServeAnotherConnection>) -> websocat_api::Result<websocat_api::Bipipe> {
        Err(websocat_api::anyhow::anyhow!("nimpl"))
    }
}


#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name = "bar",
)]
struct Bar {
}

impl websocat_api::sync::Node for Bar {
    fn run(&self, ctx: websocat_api::RunContext, allow_multiconnect: bool, mut closure: impl FnMut(websocat_api::sync::Bipipe) -> websocat_api::Result<()> + Send + 'static ) -> websocat_api::Result<()> {
        let (r,mut w2) = pipe::pipe();
        let w = std::io::sink();
        std::thread::spawn(move|| {
            closure(websocat_api::sync::Bipipe {
                r: websocat_api::sync::Source::ByteStream(Box::new(r)),
                w: websocat_api::sync::Sink::ByteStream(Box::new(w)),
                closing_notification: None,
            });
        });
        std::thread::spawn(move|| {
            for _ in 0..10 {
                use std::io::Write;
                w2.write_all(b"Qqq\n").unwrap();
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
    reg.register::<websocat_basic::io_std::Stdio>();

    //println!("{:?}", reg);

    let c = websocat_api::Session::build_from_two_tree_strings(
        &reg, 
        "[bar]",
        "[stdio]",
    ).unwrap();

    println!("{}", websocat_api::StringyNode::reverse(c.left, &c.nodes).unwrap());
    println!("{}", websocat_api::StringyNode::reverse(c.right, &c.nodes).unwrap());
    

    if let Err(e) = websocat_session::run(c).await {
        eprintln!("Error: {:#}", e);
    }
}
