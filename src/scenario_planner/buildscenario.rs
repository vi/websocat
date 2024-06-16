use std::net::{IpAddr, SocketAddr};

use http::Uri;


use super::{scenarioprinter::ScenarioPrinter, types::{CopyingType, Endpoint, Overlay, WebsocatInvocation}};

impl WebsocatInvocation {
    pub fn build_scenario(self) -> anyhow::Result<String> {
        let mut printer = ScenarioPrinter::new();

        let mut left : String;
        let mut right : String;

        left = self.left.innermost.begin_print(&mut printer)?;

        for ovl in &self.left.overlays {
            left = ovl.begin_print(&mut printer, &left)?;
        }

        right = self.right.innermost.begin_print(&mut printer)?;

        for ovl in &self.right.overlays {
            right = ovl.begin_print(&mut printer, &left)?;
        }

        match self.get_copying_type() {
            CopyingType::ByteStream => {
                printer.print_line(&format!("exchange_bytes(#{{}}, {left}, {right})"));
            }
            CopyingType::Datarams => {
                printer.print_line(&format!("exchange_packets(#{{}}, {left}, {right})"));
            }
        }
        
        for ovl in self.right.overlays.iter().rev() {
            ovl.end_print(&mut printer);
        }

        self.right.innermost.end_print(&mut printer);

        for ovl in self.left.overlays.iter().rev() {
            ovl.end_print(&mut printer);
        }

        self.left.innermost.end_print(&mut printer);

        Ok(printer.into_result())
    }
}


impl Endpoint {
    fn begin_print(&self, printer: &mut ScenarioPrinter) -> anyhow::Result<String> {
        match self {
            Endpoint::TcpConnectByIp(addr) => {
                let varnam = printer.getnewvarname("tcp");
                printer.print_line(&format!("connect_tcp(#{{addr: \"{addr}\"}}, |{varnam}| {{"));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::TcpListen(addr) => {
                let varnam = printer.getnewvarname("tcp");
                let fromaddr = printer.getnewvarname("from");
                printer.print_line(&format!("listen_tcp(#{{autospawn: true, addr: \"{addr}\"}}, |{varnam}, {fromaddr}| {{"));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::WsUrl(u) => {
                let mut parts = u.clone().into_parts();
                let auth = parts.authority.take().unwrap();
                let (mut host, port) = (auth.host(), auth.port_u16().unwrap_or(80));

                if host.starts_with('[') && host.ends_with(']') {
                    host = host.strip_prefix('[').unwrap().strip_suffix(']').unwrap();
                }

                let Ok(ip) : Result<IpAddr, _> = host.parse() else {
                    anyhow::bail!("Hostnames not supported yet")
                };

                let addr = SocketAddr::new(ip, port);

                let tcp = printer.getnewvarname("tcp");
                printer.print_line(&format!("connect_tcp(#{{addr: \"{addr}\"}}, |{tcp}| {{"));
                printer.increase_indent();

                let wsframes = printer.getnewvarname("wsframes");

                parts.scheme = None;

                let newurl = Uri::from_parts(parts).unwrap();
                printer.print_line(&format!("ws_upgrade({tcp}, #{{url: \"{newurl}\"}}, |{wsframes}| {{"));
                printer.increase_indent();

                let ws = printer.getnewvarname("ws");
                printer.print_line(&format!("let {ws} = ws_wrap(#{{client: true}}, {wsframes});"));

                Ok(ws)
            }
            Endpoint::WssUrl(_) => todo!(),
            Endpoint::Stdio => {
                let varnam = printer.getnewvarname("stdio");
                printer.print_line(&format!("let {varnam} = create_stdio();"));
                Ok(varnam)
            }
            Endpoint::UdpConnect(_) => todo!(),
            Endpoint::UdpBind(_) => todo!(),
        }
    }
    fn end_print(&self, printer: &mut ScenarioPrinter) {
        match self {
            Endpoint::TcpConnectByIp(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::TcpListen(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            },
            Endpoint::WsUrl(_) => {
                printer.decrease_indent();
                printer.print_line("})");
                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::WssUrl(_) => todo!(),
            Endpoint::Stdio => {

            }
            Endpoint::UdpConnect(_) => todo!(),
            Endpoint::UdpBind(_) => todo!(),
        }
    }
}

impl Overlay {
    fn begin_print(&self, printer: &mut ScenarioPrinter, inner_var: &str) -> anyhow::Result<String> {
        match self {
            Overlay::WsUpgrade(_) => todo!(),
            Overlay::WsWrap => todo!(),
            Overlay::StreamChunks => {
                let varnam = printer.getnewvarname("chunks");
                printer.print_line(&format!("let {varnam} = stream_chunks({inner_var});"));
                Ok(varnam)
            }
        }
    }
    fn end_print(&self, _printer: &mut ScenarioPrinter) {
        match self {
            Overlay::WsUpgrade(_) => todo!(),
            Overlay::WsWrap => todo!(),
            Overlay::StreamChunks => (),
        }
    }
}
