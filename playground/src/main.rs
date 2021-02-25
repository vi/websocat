#![allow(unused)]

use std::str::FromStr;

#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name = "foo",
    prefix="foo",
    prefix="f",
    debug_derive,
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
impl websocat_api::ParsedNode for Foo {
    async fn run(&self, ctx: websocat_api::RunContext, multiconn: &mut websocat_api::IWantToServeAnotherConnection) -> websocat_api::Result<websocat_api::Pipe> {
        Err(websocat_api::anyhow::anyhow!("nimpl"))
    }
}


#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name = "foo",
)]
struct Bar {
}

#[websocat_api::async_trait::async_trait]
impl websocat_api::ParsedNode for Bar {
    async fn run(&self, ctx: websocat_api::RunContext, multiconn: &mut websocat_api::IWantToServeAnotherConnection) -> websocat_api::Result<websocat_api::Pipe> {
        Err(websocat_api::anyhow::anyhow!("nimpl"))
    }
}


fn main() {
    let mut m = std::collections::HashMap::new();
    m.insert("foo".to_owned(), Box::new(FooClass) as websocat_api::DNodeClass);
    m.insert("bar".to_owned(), Box::new(BarClass) as websocat_api::DNodeClass);
    let mut t = websocat_api::Tree::new();

    let q = websocat_api::StringyNode::from_str("[foo o=3 inner=[bar] o=5]").unwrap();
    let w = q.build(&m, &mut t).unwrap();

    println!("{}", websocat_api::StringyNode::reverse(w, &t).unwrap());
}
