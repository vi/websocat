use crate::cli::WebsocatArgs;

use super::{
    buildscenario_exec::format_osstr,
    scenarioprinter::{ScenarioPrinter, StrLit},
    types::Endpoint,
    utils::IdentifierGenerator,
};

impl Endpoint {
    pub(super) fn begin_print_unix(
        &self,
        printer: &mut ScenarioPrinter,
        vars: &mut IdentifierGenerator,
        opts: &WebsocatArgs,
    ) -> anyhow::Result<String> {
        match self {
            Endpoint::UnixConnect(path) => {
                let varnam = vars.getnewvarname("unix");
                let pathvar = vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    printer.print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    printer.print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }
                printer.print_line(&format!("connect_unix(#{{}}, {pathvar}, |{varnam}| {{",));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::UnixListen(path) => {
                let pathvar = vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    printer.print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    printer.print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }

                if opts.unlink {
                    printer.print_line(&format!("unlink_file({pathvar}, false);"));
                }

                let varnam = vars.getnewvarname("unix");

                let mut chmod_option = "";

                if opts.chmod_owner {
                    chmod_option = ", chmod: 0o600";
                } else if opts.chmod_group {
                    chmod_option = ", chmod: 0o660";
                } else if opts.chmod_everyone {
                    chmod_option = ", chmod: 0o666";
                }

                printer.print_line(&format!(
                    "listen_unix(#{{autospawn: true {chmod_option} }}, {pathvar}, |{varnam}| {{",
                ));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::AbstractConnect(path) => {
                let varnam = vars.getnewvarname("unix");
                let pathvar = vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    printer.print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    printer.print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }
                printer.print_line(&format!("connect_unix(#{{abstract:true}}, {pathvar}, |{varnam}| {{",));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::AbstractListen(path) => {
                let pathvar = vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    printer.print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    printer.print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }

                let varnam = vars.getnewvarname("unix");

                printer.print_line(&format!(
                    "listen_unix(#{{abstract: true, autospawn: true }}, {pathvar}, |{varnam}| {{",
                ));
                printer.increase_indent();
                Ok(varnam)
            }
            _ => panic!(),
        }
    }

    pub(super) fn end_print_unix(&self, printer: &mut ScenarioPrinter) {
        match self {
            Endpoint::UnixConnect(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::UnixListen(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::AbstractConnect(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::AbstractListen(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            _ => panic!(),
        }
    }
}
