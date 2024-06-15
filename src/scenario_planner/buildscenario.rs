use crate::cli::WebsocatArgs;

use super::{scenarioprinter::ScenarioPrinter, types::{Endpoint, SpecifierStack}};

pub fn build_scenario(left: SpecifierStack, right: SpecifierStack, opts: WebsocatArgs) -> anyhow::Result<String> {
    let mut printer = ScenarioPrinter::new();

    let left_inner = left.innermost.begin_print(&mut printer);

    let right_inner = right.innermost.begin_print(&mut printer);

    printer.print_line(&format!("copy_bytes_bidirectional({left_inner}, {right_inner})"));

    right.innermost.end_print(&mut printer);

    left.innermost.end_print(&mut printer);

    Ok(printer.into_result())
}

impl Endpoint {
    fn begin_print(&self, printer: &mut ScenarioPrinter) -> String {
        match self {
            Endpoint::TcpConnectByIp(addr) => {
                let varnam = printer.getnewvarname("tcp");
                printer.print_line(&format!("connect_tcp(#{{addr: \"{addr}\"}}, |{varnam}| {{"));
                printer.increase_indent();
                varnam
            }
            Endpoint::TcpListen(_) => todo!(),
            Endpoint::WsUrl(_) => todo!(),
            Endpoint::WssUrl(_) => todo!(),
            Endpoint::Stdio => {
                let varnam = printer.getnewvarname("stdio");
                printer.print_line(&format!("let {varnam} = create_stdio();"));
                varnam
            }
            Endpoint::UdpConnect(_) => todo!(),
            Endpoint::UdpBind(_) => todo!(),
        }
    }
    fn end_print(&self, printer: &mut ScenarioPrinter) {
        match self {
            Endpoint::TcpConnectByIp(_) => {
                printer.decrease_indent();
                printer.print_line("})")
            }
            Endpoint::TcpListen(_) => todo!(),
            Endpoint::WsUrl(_) => todo!(),
            Endpoint::WssUrl(_) => todo!(),
            Endpoint::Stdio => {

            }
            Endpoint::UdpConnect(_) => todo!(),
            Endpoint::UdpBind(_) => todo!(),
        }
    }
}
