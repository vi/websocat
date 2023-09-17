#![cfg_attr(feature="cargo-clippy", allow(collapsible_if,needless_pass_by_value))]

use super::{Options, Result, SpecifierClass, SpecifierStack, WebsocatConfiguration2};
use super::specifier::{SpecifierNode};
use std::rc::Rc;
use std::str::FromStr;
use std::ops::Not;

extern crate hyper;
extern crate url;

use std::net::{IpAddr, SocketAddr};

use super::socks5_peer::{SocksHostAddr, SocksSocketAddr};

#[derive(Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
pub enum StdioUsageStatus {
    /// Does not use standard input or output at all
    None,
    /// Uses a reuser for connecting multiple peers at stdio, not distinguishing between IsItself and Indirectly
    WithReuser,
    /// Stdio wrapped into something (but not the reuser)
    Indirectly,
    /// Is the `-` or `stdio:` or `threadedstdio:` itself.
    IsItself,
}

trait ClassExt {
    fn is_stdio(&self) -> bool;
    fn is_reuser(&self) -> bool;
}

pub type OnWarning = Box<dyn for<'a> Fn(&'a str) -> () + 'static>;

#[cfg_attr(rustfmt, rustfmt_skip)]
impl ClassExt for Rc<dyn SpecifierClass> {
    fn is_stdio(&self) -> bool {
        [
            "StdioClass",
            "ThreadedStdioClass",
            "ThreadedStdioSubstituteClass",
        ].contains(&self.get_name())
    }
    fn is_reuser(&self) -> bool {
        [
            "ReuserClass",
            "BroadcastReuserClass",
        ].contains(&self.get_name())
    }
}

pub trait SpecifierStackExt {
    fn stdio_usage_status(&self) -> StdioUsageStatus;
    fn reuser_count(&self) -> usize;
    fn contains(&self, t: &'static str) -> bool;
    fn is_multiconnect(&self) -> bool;
    fn is_stream_oriented(&self) -> bool;
    fn autotoreconn_misuse(&self)  -> bool;
    fn insert_line_class_in_proper_place(&mut self, x: Rc<dyn SpecifierClass>);
}
impl SpecifierStackExt for SpecifierStack {
    fn stdio_usage_status(&self) -> StdioUsageStatus {
        use self::StdioUsageStatus::*;

        if !self.addrtype.cls.is_stdio() {
            return None;
        }

        let mut sus: StdioUsageStatus = IsItself;

        for overlay in self.overlays.iter().rev() {
            if overlay.cls.is_reuser() {
                sus = WithReuser;
            } else if sus == IsItself {
                sus = Indirectly;
            }
        }
        sus
    }
    fn reuser_count(&self) -> usize {
        let mut c = 0;
        for overlay in &self.overlays {
            if overlay.cls.is_reuser() {
                c += 1;
            }
        }
        c
    }
    fn contains(&self, t: &'static str) -> bool {
        for overlay in &self.overlays {
            if overlay.cls.get_name() == t {
                return true;
            }
        }
        self.addrtype.cls.get_name() == t
    }
    fn autotoreconn_misuse(&self)  -> bool {
        let mut autoreconnect_found = false;
        for overlay in &self.overlays {
            if overlay.cls.get_name() == "AutoReconnectClass" {
                autoreconnect_found = true;
            }
            if overlay.cls.get_name() == "BroadcastReuserClass" || overlay.cls.get_name() == "ReuserClass" {
                if autoreconnect_found {
                    return true;
                }
            }
        }
        false
    }
    fn is_multiconnect(&self) -> bool {
        use super::ClassMulticonnectStatus::*;
        match self.addrtype.cls.multiconnect_status() {
            MultiConnect => (),
            SingleConnect => return false,
            MulticonnectnessDependsOnInnerType => unreachable!(),
        }
        for overlay in self.overlays.iter().rev() {
            match overlay.cls.multiconnect_status() {
                MultiConnect => (),
                SingleConnect => return false,
                MulticonnectnessDependsOnInnerType => (),
            }
        }
        true
    }
    fn is_stream_oriented(&self) -> bool {
        use super::ClassMessageBoundaryStatus::*;
        let mut q = match self.addrtype.cls.message_boundary_status() {
            StreamOriented => true,
            MessageOriented => false,
            MessageBoundaryStatusDependsOnInnerType => unreachable!(),
        };
        for overlay in self.overlays.iter().rev() {
            match overlay.cls.message_boundary_status() {
                StreamOriented => q = true,
                MessageOriented => q = false,
                MessageBoundaryStatusDependsOnInnerType => (),
            }
        }
        q
    }
    fn insert_line_class_in_proper_place(&mut self, x: Rc<dyn SpecifierClass>) {
        use super::ClassMessageBoundaryStatus::*;
        let mut insert_idx = 0;
        for overlay in &self.overlays {
            match overlay.cls.message_boundary_status() {
                StreamOriented => break,
                MessageOriented => break,
                MessageBoundaryStatusDependsOnInnerType => insert_idx += 1,
            }
        }
        self.overlays.insert(insert_idx, SpecifierNode{cls: x});
    }
}

impl WebsocatConfiguration2 {
    pub fn inetd_mode(&self) -> bool {
        self.contains_class("InetdClass")
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[cfg_attr(feature="cargo-clippy", allow(nonminimal_bool))]
    pub fn websocket_used(&self) -> bool {
        false 
        || self.contains_class("WsConnectClass")
        || self.contains_class("WsClientClass")
        || self.contains_class("WsClientSecureClass")
        || self.contains_class("WsServerClass")
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[cfg_attr(feature="cargo-clippy", allow(nonminimal_bool))]
    pub fn exec_used(&self) -> bool {
        false 
        || self.contains_class("ExecClass")
        || self.contains_class("CmdClass")
        || self.contains_class("ShCClass")
    }

    pub fn contains_class(&self, x: &'static str) -> bool {
        self.s1.contains(x) || self.s2.contains(x)
    }

    pub fn get_exec_parameter(&self) -> Option<&str> {
        if self.s1.addrtype.cls.get_name() == "ExecClass" {
            return Some(self.s1.addr.as_str());
        }
        if self.s2.addrtype.cls.get_name() == "ExecClass" {
            return Some(self.s2.addr.as_str());
        }
        None
    }

    fn l_stdio(&mut self, multiconnect: bool, reuser_has_been_inserted: &mut bool, r#async: bool) -> Result<()> {
        use self::StdioUsageStatus::{Indirectly, IsItself, None, WithReuser};
        match (self.s1.stdio_usage_status(), self.s2.stdio_usage_status()) {
            (_, None) => (),
            (None, WithReuser) => (),
            (None, IsItself) | (None, Indirectly) => {
                if multiconnect {
                    self.s2.overlays.insert(
                        0,
                        SpecifierNode{cls:Rc::new(super::broadcast_reuse_peer::BroadcastReuserClass)},
                    );
                    *reuser_has_been_inserted = true;
                }
            }
            (IsItself, IsItself) => {
                info!("Special mode, exception from usual one-stdio rule. Acting like `cat(1)`");
                self.s2 = SpecifierStack::from_str("mirror:")?;
                if self.opts.unidirectional ^ self.opts.unidirectional_reverse {
                    self.opts.unidirectional = false;
                    self.opts.unidirectional_reverse = false;
                }
                return Ok(());
            }
            (_, _) => {
                Err("Too many usages of stdin/stdout. Specify it either on left or right address, not on both.")?;
            }
        }

        #[cfg(all(unix, feature = "unix_stdio"))]
        {
            if r#async {
                if self.s1.addrtype.cls.get_name() == "StdioClass" {
                    debug!("Substituting StdioClass with AsyncStdioClass at the left");
                    self.s1.addrtype = SpecifierNode{cls:Rc::new(crate::stdio_peer::AsyncStdioClass)};
                } 
                if self.s2.addrtype.cls.get_name() == "StdioClass" {
                    debug!("Substituting StdioClass with AsyncStdioClass at the right");
                    self.s2.addrtype = SpecifierNode{cls:Rc::new(crate::stdio_peer::AsyncStdioClass)};
                } 
            }
        }

        Ok(())
    }

    fn l_reuser(&mut self, reuser_has_been_inserted: bool) -> Result<()> {
        if self.s1.reuser_count() + self.s2.reuser_count() > 1 {
            if reuser_has_been_inserted {
                error!("The reuser you specified conflicts with automatically inserted reuser based on usage of stdin/stdout in multiconnect mode.");
            }
            Err("Too many usages of connection reuser. Please limit to only one instance.")?;
        }
        Ok(())
    }

    fn l_linemode(&mut self) -> Result<()> {
        if !self.opts.no_auto_linemode && self.opts.websocket_text_mode {
            match (self.s1.is_stream_oriented(), self.s2.is_stream_oriented()) {
                (false, false) => {}
                (true, true) => {}
                (true, false) => {
                    info!("Auto-inserting the line mode");
                    self.s1.insert_line_class_in_proper_place(Rc::new(
                        super::line_peer::Line2MessageClass,
                    ));
                    self.s2.insert_line_class_in_proper_place(Rc::new(
                        super::line_peer::Message2LineClass,
                    ));
                }
                (false, true) => {
                    info!("Auto-inserting the line mode");
                    self.s2.insert_line_class_in_proper_place(Rc::new(
                        super::line_peer::Line2MessageClass,
                    ));
                    self.s1.insert_line_class_in_proper_place(Rc::new(
                        super::line_peer::Message2LineClass,
                    ));
                }
            }
        };
        Ok(())
    }
    fn l_listener_on_the_right(&mut self, on_warning: &OnWarning) -> Result<()> {
        if !self.opts.oneshot && self.s2.is_multiconnect() && !self.s1.is_multiconnect() {
            on_warning("You have specified a listener on the right (as the second positional argument) instead of on the left. It will only serve one connection.\nChange arguments order to enable multiple parallel connections or use --oneshot argument to make single connection explicit.");
        }
        Ok(())
    }
    fn l_reuser_for_append(&mut self, multiconnect: bool) -> Result<()> {
        if multiconnect
            && (self.s2.addrtype.cls.get_name() == "WriteFileClass"
                || self.s2.addrtype.cls.get_name() == "AppendFileClass")
            && self.s2.reuser_count() == 0
        {
            info!("Auto-inserting the reuser");
            self.s2
                .overlays
                .push(SpecifierNode{cls:Rc::new(super::primitive_reuse_peer::ReuserClass)});
        };
        Ok(())
    }
    fn l_exec(&mut self, on_warning: &OnWarning) -> Result<()> {
        if self.s1.addrtype.cls.get_name() == "ExecClass" && self.s2.addrtype.cls.get_name() == "ExecClass"
        {
            Err("Can't use exec: more than one time. Replace one of them with sh-c: or cmd:.")?;
        }

        if let Some(x) = self.get_exec_parameter() {
            if self.opts.exec_args.is_empty() && x.contains(' ') {
                on_warning("Warning: you specified exec: without the corresponding --exec-args at the end of command line. Unlike in cmd: or sh-c:, spaces inside exec:'s direct parameter are interpreted as part of program name, not as separator.");
            }
        }
        Ok(())
    }
    fn l_uri_staticfiles(&mut self, on_warning: &OnWarning) -> Result<()> {
        if self.opts.restrict_uri.is_some() && !self.contains_class("WsServerClass") {
            on_warning("--restrict-uri is meaningless without a WebSocket server");
        }

        if !self.opts.serve_static_files.is_empty() && !self.contains_class("WsServerClass") {
            on_warning("--static-file (-F) is meaningless without a WebSocket server");
        }

        for sf in &self.opts.serve_static_files {
            if !sf.uri.starts_with('/') {
                on_warning(&format!(
                    "Static file's URI `{}` should begin with `/`?",
                    sf.uri
                ));
            }
            if !sf.file.exists() {
                on_warning(&format!("File {:?} does not exist", sf.file));
            }
            if !sf.content_type.contains('/') {
                on_warning(&format!(
                    "Content-Type `{}` lacks `/` character",
                    sf.content_type
                ));
            }
        }
        Ok(())
    }
    fn l_environ(&mut self, on_warning: &OnWarning) -> Result<()> {
        if self.opts.exec_set_env {
            if !self.exec_used() {
                on_warning("-e (--set-environment) is meaningless without a exec: or sh-c: or cmd: address");
            }
            if !self.contains_class("TcpListenClass") && !self.contains_class("WsServerClass") {
                on_warning("-e (--set-environment) is currently meaningless without a websocket server and/or TCP listener");
            }
        }

        if !self.opts.headers_to_env.is_empty() && !self.opts.exec_set_env {
            on_warning("--header-to-env is meaningless without -e (--set-environment)");
        }

        Ok(())
    }
    fn l_closebug(&mut self, on_warning: &OnWarning) -> Result<()> {
        if !self.opts.oneshot && self.s1.is_multiconnect() {
            if self.s1.contains("TcpListenClass")
                || self.s1.contains("UnixListenClass")
                || self.s1.contains("SeqpacketListenClass")
            {
                if !self.opts.unidirectional
                    && (self.opts.unidirectional_reverse || !self.opts.exit_on_eof)
                {
                    on_warning("Unfortunately, serving multiple clients without --exit-on-eof (-E) or with -U option is prone to socket leak in this websocat version");
                }
            }
        }
        Ok(())
    }

    fn l_socks5_c(
        s: &mut SpecifierStack,
        opts: &mut Options,
        on_warning: &OnWarning,
        secure: bool,
    ) -> Result<()> {
        let url = if secure {
            #[cfg(not(feature = "ssl"))]
            {
                Err("SSL support not compiled in")?;
            }
            format!("wss://{}", s.addr)
        } else {
            format!("ws://{}", s.addr)
        };

        // Overwrite WsClientClass
        s.addrtype = SpecifierNode{cls: Rc::new(super::net_peer::TcpConnectClass) };

        match opts.auto_socks5.unwrap() {
            SocketAddr::V4(sa4) => {
                s.addr = format!("{}:{}", sa4.ip(), sa4.port());
            }
            SocketAddr::V6(sa6) => {
                s.addr = format!("[{}]:{}", sa6.ip(), sa6.port());
            }
        }

        use self::hyper::Url;
        use self::url::Host;
        let u = Url::parse(&url)?;

        if !u.has_host() {
            Err("WebSocket URL has no host")?;
        }

        let port = u.port_or_known_default().unwrap_or(80);
        let host = u.host().unwrap();

        let host = match host {
            Host::Domain(dom) => SocksHostAddr::Name(dom.to_string()),
            Host::Ipv4(ip4) => SocksHostAddr::Ip(IpAddr::V4(ip4)),
            Host::Ipv6(ip6) => SocksHostAddr::Ip(IpAddr::V6(ip6)),
        };
        if opts.socks_destination.is_none() {
            opts.socks_destination = Some(SocksSocketAddr { host, port });
        }
        if secure && opts.tls_domain.is_none() {
            opts.tls_domain = u.host_str().map(|x| x.to_string());
        }

        if opts.ws_c_uri != "ws://0.0.0.0/" {
            on_warning(
                "Looks like you've overridden ws-c-uri. We are overwriting it for --socks5 option.",
            );
        }

        opts.ws_c_uri = url;

        s.overlays
            .push(SpecifierNode{cls: Rc::new(super::ws_client_peer::WsConnectClass)});
        if secure {
            #[cfg(feature = "ssl")]
            s.overlays.push(SpecifierNode{cls: Rc::new(super::ssl_peer::TlsConnectClass)});
        }
        s.overlays.push(SpecifierNode{cls: Rc::new(super::socks5_peer::SocksProxyClass)});

        Ok(())
    }

    fn l_socks5(&mut self, on_warning: &OnWarning) -> Result<()> {
        if self.opts.socks_destination.is_some()
            ^ (self.contains_class("SocksProxyClass") || self.contains_class("SocksBindClass"))
        {
            on_warning(
                "--socks5-destination option and socks5-connect: overlay should go together",
            );
        }

        if self.opts.socks5_bind_script.is_some() ^ self.contains_class("SocksBindClass") {
            on_warning("--socks5-bind-script option and socks5-bind: overlay should go together");
        }

        if self.opts.auto_socks5.is_some() {
            if !((self.s1.addrtype.cls.get_name() == "WsClientClass"
                || self.s1.addrtype.cls.get_name() == "WsClientSecureClass")
                ^ (self.s2.addrtype.cls.get_name() == "WsClientClass"
                    || self.s2.addrtype.cls.get_name() == "WsClientSecureClass"))
            {
                Err("User-friendly --socks5 option supports socksifying exactly one non-raw websocket client connection. You are using two or none.")?;
            }

            if self.s1.addrtype.cls.get_name() == "WsClientClass" {
                WebsocatConfiguration2::l_socks5_c(
                    &mut self.s1,
                    &mut self.opts,
                    on_warning,
                    false,
                )?;
            }
            if self.s1.addrtype.cls.get_name() == "WsClientSecureClass" {
                WebsocatConfiguration2::l_socks5_c(&mut self.s1, &mut self.opts, on_warning, true)?;
            }
            if self.s2.addrtype.cls.get_name() == "WsClientClass" {
                WebsocatConfiguration2::l_socks5_c(
                    &mut self.s2,
                    &mut self.opts,
                    on_warning,
                    false,
                )?;
            }
            if self.s2.addrtype.cls.get_name() == "WsClientSecureClass" {
                WebsocatConfiguration2::l_socks5_c(&mut self.s2, &mut self.opts, on_warning, true)?;
            }
        }
        Ok(())
    }

    #[cfg(feature = "ssl")]
    fn l_ssl(&mut self, _on_warning: &OnWarning) -> Result<()> {
        if self.opts.pkcs12_der.is_some() &&  !self.contains_class("TlsAcceptClass") {
            Err("--pkcs12-der makes no sense without an TLS connections acceptor")?;
        }
        if self.opts.pkcs12_der.is_none() &&  self.contains_class("TlsAcceptClass") {
            Err("You need to specify server key and certificate using the --pkcs12-der option to use the TLS connections acceptor")?;
        }
        if self.opts.client_pkcs12_der.is_some() && !self.contains_class("WsClientSecureClass") && !self.contains_class("TlsConnectClass") {
            Err("--client-pkcs12-der makes no sense without wss:// or ssl: connectors")?;
        }
        #[cfg(target_os = "macos")]
        {
            if (self.opts.pkcs12_der.is_some() && self.opts.pkcs12_passwd.is_none()) || (self.opts.client_pkcs12_der.is_some() && self.opts.client_pkcs12_passwd.is_none()) {
                _on_warning("PKCS12 archives without password may be unsupported on Mac");

                for x in ::std::env::args() {
                    if x.contains("test.pkcs12") {
                        _on_warning("If you want a pre-made test certificate, use other file: `--pkcs12-der 1234.pkcs12 --pkcs12-passwd 1234`");
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    fn l_ping(&mut self, _on_warning: &OnWarning) -> Result<()> {
        if self.opts.ws_ping_interval.is_some() || self.opts.ws_ping_timeout.is_some() {
            if !self.websocket_used() {
                _on_warning("--ping-interval or --ping-timeout options are not effective if no WebSocket usage is specified")
            }
        }
        if self.opts.ws_ping_timeout.is_some() && self.opts.ws_ping_interval.is_none() {
            _on_warning("--ping-timeout specified without --ping-interval. This will probably lead to unconditional disconnection after that interval.")
        }
        if let (Some(t), Some(i)) = (self.opts.ws_ping_timeout, self.opts.ws_ping_interval) {
            if t <= i {
                _on_warning("--ping-timeout's value is not more than --ping-interval. Expect spurious disconnections.");
            }
        }
        if self.opts.ws_ping_timeout.is_some() {
            if self.opts.unidirectional_reverse || self.opts.exit_on_eof {
                // OK
            } else {
                _on_warning("--ping-interval is currently not very effective without -E or -U")
            }
        }
        if self.opts.print_ping_rtts && self.opts.ws_ping_interval.is_none() {
            _on_warning("--print-ping-rtts is not effective without --ping-interval");
        }
        Ok(())
    }

    fn l_proto(&mut self, _on_warning: &OnWarning) -> Result<()> {
        if self.opts.websocket_protocol.is_some() {
            if false
                || self.contains_class("WsConnectClass")
                || self.contains_class("WsClientClass")
                || self.contains_class("WsClientSecureClass")
            {
                // OK
            } else {
                if self.contains_class("WsServerClass") {
                    _on_warning("--protocol option is unused. Maybe you want --server-protocol?")
                } else {
                    _on_warning("--protocol option is unused.")
                }
            }
        }
        if self.opts.websocket_reply_protocol.is_some() {
            if !self.contains_class("WsServerClass") {
                _on_warning("--server-protocol option is unused.")
            }
        }
        Ok(())
    }

    fn l_eeof_unidir(&mut self, _on_warning: &OnWarning) -> Result<()> {
        if self.opts.exit_on_eof {
           if self.opts.unidirectional || self.opts.unidirectional_reverse {
               _on_warning("--exit-on-eof and --unidirectional[-reverse] options are now useless together");
               _on_warning("You may want to remove --exit-on-eof. If you are happy with what happens, consider `-uU` instead of `-uE`.");
           }
        }
        Ok(())
    }

    fn l_udp(&mut self, _on_warning: &OnWarning) -> Result<()> {
        if self.opts.udp_join_multicast_addr.is_empty().not() {
            if self.opts.udp_broadcast {
                _on_warning("Both --udp-broadcast and a multicast address is set. This is strange.");
            }
            let ifs = self.opts.udp_join_multicast_iface_v4.len() + self.opts.udp_join_multicast_iface_v6.len();
            if ifs != 0 {
                let mut v4_multicasts = 0;
                let mut v6_multicasts = 0;
                for i in &self.opts.udp_join_multicast_addr {
                    match i {
                        std::net::IpAddr::V4(_) => v4_multicasts += 1,
                        std::net::IpAddr::V6(_) => v6_multicasts += 1,
                    }
                }
                if v4_multicasts != self.opts.udp_join_multicast_iface_v4.len() {
                    return Err("--udp-multicast-iface-v4 option mush be specified the same number of times as IPv4 addresses for --udp-multicast (alternatively --udp-multicast-iface-* options should be not specified at all)")?;
                }
                if v6_multicasts != self.opts.udp_join_multicast_iface_v6.len() {
                    return Err("--udp-multicast-iface-v6 option mush be specified the same number of times as IPv6 addresses for --udp-multicast (alternatively --udp-multicast-iface-* options should be not specified at all)")?;
                }
            }
        } else {
            if self.opts.udp_multicast_loop {
                return Err("--udp-multicast-loop is not applicable without --udp-multicast")?;
            }
        }
        Ok(())
    }
    fn l_crypto(&mut self, _on_warning: &OnWarning) -> Result<()> {
        #[cfg(feature="crypto_peer")]
        if self.opts.crypto_key.is_some() {
            if !self.contains_class("CryptoClass") {
                _on_warning("--crypto-key option is meaningless without a `crypto:` overlay");
            }
        }
        Ok(())
    }
    fn l_prometheus(&mut self, _on_warning: &OnWarning) -> Result<()> {
        #[cfg(feature="prometheus_peer")]
        if self.opts.prometheus.is_some() {
            if !self.contains_class("PrometheusClass") {
                self.s2.overlays.insert(0, SpecifierNode { cls: Rc::new(crate::prometheus_peer::PrometheusClass) });
            }
        } else {
            if self.contains_class("PrometheusClass") {
                _on_warning("Using `prometheus:` overlay without `--prometheus` option is meaningless");
            }
        }
        Ok(())
    }
    fn l_sizelimits(&mut self, _on_warning: &OnWarning) -> Result<()> {
        if self.opts.max_ws_message_length < self.opts.max_ws_frame_length {
            _on_warning("Lowering --max-ws-message-length without also lowering --max-ws-frame-length may be meaningless, as the former only affects whether to begin accept a new frame or not, given accumulated message size. Succesfully accepted frames within the frame size limit may exceed the message size.")
        }
        Ok(())
    }
    fn l_compress(&mut self, _on_warning: &OnWarning) -> Result<()> {
        let mut cn = 0;
        let mut un = 0;
        if self.opts.compress_deflate { cn += 1 }
        if self.opts.compress_gzip { cn += 1 }
        if self.opts.compress_zlib { cn += 1 }
        if self.opts.uncompress_deflate { un += 1 }
        if self.opts.uncompress_gzip { un += 1 }
        if self.opts.uncompress_zlib { un += 1 }
        if cn > 1 {
            return Err("Multiple --compress-* options specifed")?;
        }
        if un > 1 {
            return Err("Multiple --uncompress-* options specifed")?;
        }

        #[cfg(not(feature="compression"))]
        {
            if cn > 0 || un > 0 {
                return Err("Compression support is not selected during Websocat compilation")?;
            }
        }
        Ok(())
    }
    #[cfg(feature="native_plugins")]
    fn l_plugins(&mut self, _on_warning: &OnWarning) -> Result<()> {
        if self.contains_class("NativeTransformAClass") && self.opts.native_transform_a.is_none() {
            return Err("--native-plugin-a must be specified to use `native_plugin_transform_a:`")?;
        }
        if self.contains_class("NativeTransformBClass") && self.opts.native_transform_b.is_none() {
            return Err("--native-plugin-b must be specified to use `native_plugin_transform_b:`")?;
        }
        if self.contains_class("NativeTransformCClass") && self.opts.native_transform_c.is_none() {
            return Err("--native-plugin-c must be specified to use `native_plugin_transform_c:`")?;
        }
        if self.contains_class("NativeTransformDClass") && self.opts.native_transform_d.is_none() {
            return Err("--native-plugin-d must be specified to use `native_plugin_transform_d:`")?;
        }
        Ok(())
    }
    #[cfg(not(feature="native_plugins"))]
    fn l_plugins(&mut self, _on_warning: &OnWarning) -> Result<()> {
        Ok(())
    }

    #[cfg(feature="wasm_plugins")]
    fn l_wasm(&mut self, _on_warning: &OnWarning) -> Result<()> {
        if self.contains_class("WasmTransformAClass") && self.opts.wasm_transform_a.is_none() {
            return Err("--wasm-plugin-a must be specified to use `wasm_plugin_transform_a:`")?;
        }
        if self.contains_class("WasmTransformBClass") && self.opts.wasm_transform_b.is_none() {
            return Err("--wasm-plugin-b must be specified to use `wasm_plugin_transform_b:`")?;
        }
        if self.contains_class("WasmTransformCClass") && self.opts.wasm_transform_c.is_none() {
            return Err("--wasm-plugin-c must be specified to use `wasm_plugin_transform_c:`")?;
        }
        if self.contains_class("WasmTransformDClass") && self.opts.wasm_transform_d.is_none() {
            return Err("--wasm-plugin-d must be specified to use `wasm_plugin_transform_d:`")?;
        }
        Ok(())
    }
    #[cfg(not(feature="wasm_plugins"))]
    fn l_wasm(&mut self, _on_warning: &OnWarning) -> Result<()> {
        Ok(())
    }

    fn l_autoreconn_reuse(&mut self, _on_warning: &OnWarning) -> Result<()> {
        if self.s1.autotoreconn_misuse() || self.s2.autotoreconn_misuse() {
            _on_warning("Warning: `autoreconnect:reuse:` is a bad overlay combination. Maybe you want `reuse:autoreconnect:");
        }
        Ok(())
    }

    pub fn lint_and_fixup(&mut self, on_warning: OnWarning) -> Result<()> {
        let multiconnect = !self.opts.oneshot && self.s1.is_multiconnect();
        let mut reuser_has_been_inserted = false;

        self.l_prometheus(&on_warning)?;
        self.l_stdio(multiconnect, &mut reuser_has_been_inserted, self.opts.asyncstdio)?;
        self.l_reuser(reuser_has_been_inserted)?;
        self.l_linemode()?;
        self.l_listener_on_the_right(&on_warning)?;
        self.l_reuser_for_append(multiconnect)?;
        self.l_exec(&on_warning)?;
        self.l_uri_staticfiles(&on_warning)?;
        self.l_environ(&on_warning)?;
        self.l_closebug(&on_warning)?;
        self.l_socks5(&on_warning)?;
        #[cfg(feature = "ssl")]
        self.l_ssl(&on_warning)?;
        self.l_ping(&on_warning)?;
        self.l_proto(&on_warning)?;
        self.l_eeof_unidir(&on_warning)?;
        self.l_udp(&on_warning)?;
        self.l_crypto(&on_warning)?;
        self.l_sizelimits(&on_warning)?;
        self.l_compress(&on_warning)?;
        self.l_plugins(&on_warning)?;
        self.l_wasm(&on_warning)?;
        self.l_autoreconn_reuse(&on_warning)?;

        // TODO: UDP connect oneshot mode
        // TODO: tests for the linter
        Ok(())
    }
}
