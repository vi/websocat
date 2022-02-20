use websocat_api::{anyhow::bail, stringy::StringOrSubnode, StrNode};

#[derive(Default)]
/// #[auto_populate_macro_in_allclasslist]
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

        if ! input.properties.is_empty() {
            bail!("No properties expected");
        }

        if input.array.len() != 1 {
            bail!("Input array should have length 1");
        }

        let uri = match input.array.drain(..).next().unwrap() {
            Str(x) => x,
            Subnode(_) => bail!("Array element must be string, not a subnode"),
        };

        let ret = StrNode {
            name: "session".into(),
            array: vec![],
            enable_autopopulate: true,
            properties: vec![
                (
                    "left".into(),
                    Subnode(StrNode {
                        name: "datagrams".into(),
                        array: vec![],
                        enable_autopopulate: true,
                        properties: vec![(
                            "inner".into(),
                            Subnode(StrNode {
                                name: "stdio".into(),
                                properties: vec![],
                                array: vec![],
                                enable_autopopulate: true,
                            }),
                        )],
                    }),
                ),
                (
                    "right".into(),
                    Subnode(StrNode {
                        name: "wsc".into(),
                        array: vec![],
                        enable_autopopulate: true,
                        properties: vec![(
                            "uri".into(),
                            Str(uri),
                        )],
                    }),
                ),
            ],
        };
        Ok(ret)
    }
}
