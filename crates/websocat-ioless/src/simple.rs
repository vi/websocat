use websocat_api::{
    anyhow::bail,
    stringy::{StringOrSubnode, UnquoteResult},
    StrNode,
};
use websocat_derive::WebsocatMacro;

#[derive(Default, WebsocatMacro)]
#[auto_populate_macro_in_allclasslist]
pub struct SimpleClientSession;
impl websocat_api::Macro for SimpleClientSession {
    fn official_name(&self) -> String {
        "client".to_owned()
    }
    fn injected_cli_opts(&self) -> Vec<(String, websocat_api::CliOptionDescription)> {
        vec![]
    }

    fn run(
        &self,
        mut input: StrNode,
        _opts: &websocat_api::CliOpts,
    ) -> websocat_api::Result<StrNode> {
        use StringOrSubnode::{Str, Subnode};

        if !input.properties.is_empty() {
            bail!("No properties expected");
        }

        if input.array.len() != 1 {
            bail!("Input array should have length 1");
        }

        let uri = match input.array.drain(..).next().unwrap() {
            Str(x) => x,
            Subnode(_) => bail!("Array element must be string, not a subnode"),
        };

        StrNode::quasiquote(
            b"[session left=[datagrams @ stdio] right=[wsc uri=,u +] +]",
            &|_| Ok(UnquoteResult::Bytes(uri.clone())),
        )
    }
}


#[derive(Default, WebsocatMacro)]
#[auto_populate_macro_in_allclasslist]
pub struct SimpleServerSession;
impl websocat_api::Macro for SimpleServerSession {
    fn official_name(&self) -> String {
        "server".to_owned()
    }
    fn injected_cli_opts(&self) -> Vec<(String, websocat_api::CliOptionDescription)> {
        vec![]
    }

    fn run(
        &self,
        mut input: StrNode,
        _opts: &websocat_api::CliOpts,
    ) -> websocat_api::Result<StrNode> {
        use StringOrSubnode::{Str, Subnode};

        if !input.properties.is_empty() {
            bail!("No properties expected");
        }

        if input.array.len() != 1 {
            bail!("Input array should have length 1");
        }

        let mut host_and_or_port = match input.array.drain(..).next().unwrap() {
            Str(x) => x,
            Subnode(_) => bail!("Array element must be string, not a subnode"),
        };

        if ! host_and_or_port.contains(&b':') {
            let mut tmp = websocat_api::bytes::BytesMut::with_capacity(10+host_and_or_port.len());
            tmp.extend_from_slice(b"127.0.0.1:");
            tmp.extend(host_and_or_port);
            host_and_or_port = tmp.freeze();
        }

        eprintln!("Listening {}", String::from_utf8_lossy(&host_and_or_port.to_vec()));

        StrNode::quasiquote(
            b"[session left=[wsl + @ tcp-listen ,h +] right=[reuse-broadcast + @ datagrams @ stdio]]",
            &|_| Ok(UnquoteResult::Bytes(host_and_or_port.clone())),
        )
    }
}
