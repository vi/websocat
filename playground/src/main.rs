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
    async fn run(&self, ctx: websocat_api::RunContext, multiconn: &mut websocat_api::IWantToServeAnotherConnection) -> websocat_api::Result<websocat_api::Pipe> {
        Err(websocat_api::anyhow::anyhow!("nimpl"))
    }
}


#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name = "bar",
)]
struct Bar {
}

impl websocat_api::SyncNode for Bar {
    
}


#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut reg = websocat_api::ClassRegistrar::default();
    reg.register::<Foo>();
    reg.register::<Bar>();
    reg.register::<websocat_basic::net::Tcp>();
    reg.register::<websocat_basic::io_std::Stdio>();

    //println!("{:?}", reg);

    let mut t = websocat_api::Tree::new();
    
    let q = websocat_api::StringyNode::from_str("[tcp addr=127.0.0.1:1234]").unwrap();
    let w = match q.build(&reg, &mut t) {
        Ok(x) => x,
        Err(e) => {eprintln!("Err: {:#}", e); return}
    };

    let q2 = websocat_api::StringyNode::from_str("[stdio]").unwrap();
    let w2 = match q2.build(&reg, &mut t) {
        Ok(x) => x,
        Err(e) => {eprintln!("Err: {:#}", e); return}
    };

    println!("{}", websocat_api::StringyNode::reverse(w, &t).unwrap());
    println!("{}", websocat_api::StringyNode::reverse(w2, &t).unwrap());

    let c = websocat_api::WebsocatContext::new(t, w, w2);

    let rc1 = websocat_api::RunContext {
        nodes: c.nodes.clone(),
        left_to_right_things_to_be_filled_in: None,
        left_to_right_things_to_read_from: None,
        globals: c.global_things.clone(),
    };

    let mut _dummy = websocat_api::IWantToServeAnotherConnection::None;
    let mut p1: websocat_api::Pipe = c.nodes[c.left].run(rc1, &mut _dummy).await.unwrap();

    let rc2 = websocat_api::RunContext {
        nodes: c.nodes.clone(),
        left_to_right_things_to_be_filled_in: None,
        left_to_right_things_to_read_from: None,
        globals: c.global_things.clone(),
    };
    let mut _dummy = websocat_api::IWantToServeAnotherConnection::None;

    let mut p2 : websocat_api::Pipe = c.nodes[c.right].run(rc2, &mut _dummy).await.unwrap();

    let bytes = tokio::io::copy(&mut p1.r, &mut p2.w).await.unwrap();
    println!("bytes={}", bytes);
}
