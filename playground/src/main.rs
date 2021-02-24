#![allow(unused)]

use std::str::FromStr;

/*
#[derive(websocat_derive::MyMacroHere)]
#[qqq(3)]
struct Qqq {
    #[qqq(2)]
    rr : i32,
}
*/

#[derive(Debug, websocat_derive::WebsocatNode)]
struct Foo {
    o : i64,
    inner : websocat_api::NodeId,
    t : Option<String>,
}

#[derive(Default)]
struct FooBuilder {
    o : Option<i64>,
    inner: Option<websocat_api::NodeId>,
    t: Option<String>,
}

struct FooClass;

impl websocat_api::NodeClass for FooClass {
    fn official_name(&self) -> String { "foo".to_owned() }

    fn prefixes(&self) -> Vec<String> { vec!["foo".to_owned()] }

    fn properties(&self) -> Vec<websocat_api::PropertyInfo> {
        vec![
            websocat_api::PropertyInfo {
                name: "o".to_owned(),
                r#type: websocat_api::PropertyValueType::Numbery,
                help: "o".to_owned(),
            },
            websocat_api::PropertyInfo {
                name: "t".to_owned(),
                r#type: websocat_api::PropertyValueType::Stringy,
                help: "t".to_owned(),
            },
            websocat_api::PropertyInfo {
                name: "inner".to_owned(),
                r#type: websocat_api::PropertyValueType::ChildNode,
                help: "inner".to_owned(),
            },
        ]
    }

    fn array_type(&self) -> Option<websocat_api::PropertyValueType> {
        None
    }

    fn new_node(&self) -> websocat_api::DNodeInProgressOfParsing {
        Box::new(FooBuilder::default())
    }

    fn run_lints(&self, nodeid: &websocat_api::NodeId, placement: websocat_api::NodePlacement, context: &websocat_api::WebsocatContext) -> websocat_api::Result<Vec<String>> {
        Ok(vec![])
    }
}

impl websocat_api::NodeInProgressOfParsing for FooBuilder {
    fn set_property(&mut self, name: &str, val: websocat_api::PropertyValue) -> websocat_api::Result<()> {
        use websocat_api::PropertyValue as PV;
        match (name, val) {
            ("o", PV::Numbery(n)) => self.o = Some(n),
            ("inner", PV::ChildNode(n)) => self.inner = Some(n),
            ("t", PV::Stringy(n)) => self.t = Some(n),
            _ => websocat_api::anyhow::bail!("Unknown property {} or wrong type", name),
        }
        Ok(())
    }

    fn push_array_element(&mut self, val: websocat_api::PropertyValue) -> websocat_api::Result<()> {
        websocat_api::anyhow::bail!("No array elements expected here");
    }

    fn finish(self: Box<Self>) -> websocat_api::Result<websocat_api::DParsedNode> {
        if self.o.is_none() {
            websocat_api::anyhow::bail!("Property `o` must be set");
        }
        if self.inner.is_none() {
            websocat_api::anyhow::bail!("Property `inner` must be set");
        }
        Ok(Box::pin(
            Foo {
                o : self.o.unwrap(),
                inner: self.inner.unwrap(),
                t: self.t,
            }
        ))
    }
}

#[websocat_api::async_trait::async_trait]
impl websocat_api::ParsedNode for Foo {
    async fn run(&self, ctx: websocat_api::RunContext, multiconn: &mut websocat_api::IWantToServeAnotherConnection) -> websocat_api::Result<websocat_api::Pipe> {
        Err(websocat_api::anyhow::anyhow!("nimpl"))
    }
}


#[derive(Debug)]
struct Bar {
}

#[derive(Default)]
struct BarBuilder {
}

struct BarClass;

impl websocat_api::NodeClass for BarClass {
    fn official_name(&self) -> String { "bar".to_owned() }

    fn prefixes(&self) -> Vec<String> { vec!["bar".to_owned()] }

    fn properties(&self) -> Vec<websocat_api::PropertyInfo> {
        vec![]
    }

    fn array_type(&self) -> Option<websocat_api::PropertyValueType> {
        None
    }

    fn new_node(&self) -> websocat_api::DNodeInProgressOfParsing {
        Box::new(BarBuilder::default())
    }

    fn run_lints(&self, nodeid: &websocat_api::NodeId, placement: websocat_api::NodePlacement, context: &websocat_api::WebsocatContext) -> websocat_api::Result<Vec<String>> {
        Ok(vec![])
    }
}

impl websocat_api::NodeInProgressOfParsing for BarBuilder {
    fn set_property(&mut self, name: &str, val: websocat_api::PropertyValue) -> websocat_api::Result<()> {
        use websocat_api::PropertyValue as PV;
        match (name, val) {
            _ => websocat_api::anyhow::bail!("Unknown property {} or wrong type", name),
        }
        Ok(())
    }

    fn push_array_element(&mut self, val: websocat_api::PropertyValue) -> websocat_api::Result<()> {
        websocat_api::anyhow::bail!("No array elements expected here");
    }

    fn finish(self: Box<Self>) -> websocat_api::Result<websocat_api::DParsedNode> {
        Ok(Box::pin(
            Bar {
            }
        ))
    }
}

impl websocat_api::ParsedNodeProperyAccess for Bar {
    fn class(&self) -> websocat_api::DNodeClass {
        Box::new(BarClass)
    }

    fn get_property(&self, name:&str) -> Option<websocat_api::PropertyValue> {
        match name {
            _ => None,
        }
    }

    fn get_array(&self) -> Vec<websocat_api::PropertyValue> {
        vec![]
    }
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
