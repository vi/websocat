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
