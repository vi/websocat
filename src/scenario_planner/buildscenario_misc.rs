use crate::cli::WebsocatArgs;

use super::{
    scenarioprinter::{ScenarioPrinter, StrLit},
    types::PreparatoryAction,
    utils::IdentifierGenerator,
};

impl PreparatoryAction {
    pub(super) fn begin_print(
        &self,
        printer: &mut ScenarioPrinter,
        opts: &WebsocatArgs,
        _vars: &mut IdentifierGenerator,
    ) -> anyhow::Result<()> {
        match self {
            PreparatoryAction::ResolveHostname {
                hostname,
                varname_for_addrs,
            } => {
                printer.print_line(&format!(
                    "lookup_host({hn}, |{varname_for_addrs}| {{",
                    hn = StrLit(hostname),
                ));
                printer.increase_indent();
            }
            PreparatoryAction::CreateTlsConnector {
                varname_for_connector,
            } => {
                if opts.insecure {
                    printer.print_line(&format!(
                        "let {varname_for_connector} = tls_client_connector(#{{danger_accept_invalid_certs: true, danger_accept_invalid_hostnames: true}});"
                    ));
                } else {
                    printer.print_line(&format!(
                        "let {varname_for_connector} = tls_client_connector(#{{}});"
                    ));
                }
            }
            PreparatoryAction::CreateSimpleReuserListener { varname_for_reuser } => {
                printer.print_line(&format!(
                    "let {varname_for_reuser} = simple_reuser_listener();"
                ));
            }
        }
        Ok(())
    }
    pub(super) fn end_print(&self, printer: &mut ScenarioPrinter) {
        match self {
            PreparatoryAction::ResolveHostname { .. } => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            PreparatoryAction::CreateTlsConnector { .. } => (),
            PreparatoryAction::CreateSimpleReuserListener { .. } => (),
        }
    }
}

impl WebsocatArgs {
    pub fn listening_parameters(&self) -> &'static str {
        if !self.oneshot {
            "autospawn: true, oneshot: false"
        } else {
            "autospawn: false, oneshot: true"
        }
    }
}
