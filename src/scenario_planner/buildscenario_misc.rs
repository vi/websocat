use crate::cli::WebsocatArgs;

use super::{
    scenarioprinter::StrLit,
    types::{PreparatoryAction, ScenarioPrintingEnvironment, SpecifierPosition},
};

impl PreparatoryAction {
    pub(super) fn begin_print(
        &self,
        env: &mut ScenarioPrintingEnvironment<'_>,
    ) -> anyhow::Result<()> {
        match self {
            PreparatoryAction::ResolveHostname {
                hostname,
                varname_for_addrs,
            } => {
                env.printer.print_line(&format!(
                    "lookup_host({hn}, |{varname_for_addrs}| {{",
                    hn = StrLit(hostname),
                ));
                env.printer.increase_indent();
            }
            PreparatoryAction::CreateTlsConnector {
                varname_for_connector,
            } => {
                if env.opts.insecure {
                    env.printer.print_line(&format!(
                        "let {varname_for_connector} = tls_client_connector(#{{danger_accept_invalid_certs: true, danger_accept_invalid_hostnames: true}});"
                    ));
                } else {
                    env.printer.print_line(&format!(
                        "let {varname_for_connector} = tls_client_connector(#{{}});"
                    ));
                }
            }
            PreparatoryAction::CreateSimpleReuserListener { varname_for_reuser } => {
                env.printer.print_line(&format!(
                    "let {varname_for_reuser} = simple_reuser_listener();"
                ));
            }
        }
        Ok(())
    }
    pub(super) fn end_print(&self, env: &mut ScenarioPrintingEnvironment<'_>) {
        match self {
            PreparatoryAction::ResolveHostname { .. } => {
                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
            PreparatoryAction::CreateTlsConnector { .. } => (),
            PreparatoryAction::CreateSimpleReuserListener { .. } => (),
        }
    }
}

impl WebsocatArgs {
    pub fn listening_parameters(&self, position: SpecifierPosition) -> &'static str {
        if !self.oneshot && position == SpecifierPosition::Left {
            "autospawn: true, oneshot: false"
        } else {
            "autospawn: false, oneshot: true"
        }
    }
}
