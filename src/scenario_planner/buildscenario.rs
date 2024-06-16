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
            right = ovl.begin_print(&mut printer, &right)?;
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
            Endpoint::WsUrl(..) => {
                panic!("This endpoint is supposed to be split up by specifier stack patcher before.");
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
                panic!("This endpoint is supposed to be split up by specifier stack patcher before.");
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
            Overlay::WsUpgrade{ uri, host } => {
                let wsframes = printer.getnewvarname("wsframes");

                printer.print_line(&format!("ws_upgrade({inner_var}, #{{host: \"{host}\", url: \"{uri}\"}}, |{wsframes}| {{"));
                printer.increase_indent();

                Ok(wsframes)
            }
            Overlay::WsFramer{client_mode} => {
                let ws = printer.getnewvarname("ws");
                printer.print_line(&format!("let {ws} = ws_wrap(#{{client: {client_mode}}}, {inner_var});"));

                Ok(ws)
            }
            Overlay::StreamChunks => {
                let varnam = printer.getnewvarname("chunks");
                printer.print_line(&format!("let {varnam} = stream_chunks({inner_var});"));
                Ok(varnam)
            }
        }
    }
    fn end_print(&self, printer: &mut ScenarioPrinter) {
        match self {
            Overlay::WsUpgrade{..} => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Overlay::WsFramer{..} => (),
            Overlay::StreamChunks => (),
        }
    }
}
