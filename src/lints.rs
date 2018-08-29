#![cfg_attr(feature="cargo-clippy", allow(collapsible_if))]

use super::{Result, SpecifierClass, SpecifierStack, WebsocatConfiguration2, Options};
use std::rc::Rc;
use std::str::FromStr;

extern crate hyper;
extern crate url;

use ::std::net::{SocketAddr,IpAddr};

use super::proxy_peer::{SocksHostAddr, SocksSocketAddr};

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

pub type OnWarning = Box<for<'a> Fn(&'a str) -> () + 'static>;

#[cfg_attr(rustfmt, rustfmt_skip)]
impl ClassExt for Rc<SpecifierClass> {
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
    fn insert_line_class_in_proper_place(&mut self, x: Rc<SpecifierClass>);
}
impl SpecifierStackExt for SpecifierStack {
    fn stdio_usage_status(&self) -> StdioUsageStatus {
        use self::StdioUsageStatus::*;

        if !self.addrtype.is_stdio() {
            return None;
        }

        let mut sus: StdioUsageStatus = IsItself;

        for overlay in self.overlays.iter().rev() {
            if overlay.is_reuser() {
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
            if overlay.is_reuser() {
                c += 1;
            }
        }
        c
    }
    fn contains(&self, t: &'static str) -> bool {
        for overlay in &self.overlays {
            if overlay.get_name() == t {
                return true;
            }
        }
        self.addrtype.get_name() == t
    }
    fn is_multiconnect(&self) -> bool {
        use super::ClassMulticonnectStatus::*;
        match self.addrtype.multiconnect_status() {
            MultiConnect => (),
            SingleConnect => return false,
            MulticonnectnessDependsOnInnerType => unreachable!(),
        }
        for overlay in self.overlays.iter().rev() {
            match overlay.multiconnect_status() {
                MultiConnect => (),
                SingleConnect => return false,
                MulticonnectnessDependsOnInnerType => (),
            }
        }
        true
    }
    fn is_stream_oriented(&self) -> bool {
        use super::ClassMessageBoundaryStatus::*;
        let mut q = match self.addrtype.message_boundary_status() {
            StreamOriented => true,
            MessageOriented => false,
            MessageBoundaryStatusDependsOnInnerType => unreachable!(),
        };
        for overlay in self.overlays.iter().rev() {
            match overlay.message_boundary_status() {
                StreamOriented => q = true,
                MessageOriented => q = false,
                MessageBoundaryStatusDependsOnInnerType => (),
            }
        }
        q
    }
    fn insert_line_class_in_proper_place(&mut self, x: Rc<SpecifierClass>) {
        use super::ClassMessageBoundaryStatus::*;
        let mut insert_idx = 0;
        for overlay in &self.overlays {
            match overlay.message_boundary_status() {
                StreamOriented => break,
                MessageOriented => break,
                MessageBoundaryStatusDependsOnInnerType => insert_idx += 1,
            }
        }
        self.overlays.insert(insert_idx, x);
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
        if self.s1.addrtype.get_name() == "ExecClass" {
            return Some(self.s1.addr.as_str());
        }
        if self.s2.addrtype.get_name() == "ExecClass" {
            return Some(self.s2.addr.as_str());
        }
        None
    }

    fn l_stdio(&mut self, multiconnect: bool, reuser_has_been_inserted: &mut bool) -> Result<()> {
        use self::StdioUsageStatus::{Indirectly, IsItself, None, WithReuser};
        match (self.s1.stdio_usage_status(), self.s2.stdio_usage_status()) {
            (_, None) => (),
            (None, WithReuser) => (),
            (None, IsItself) | (None, Indirectly) => {
                if multiconnect {
                    self.s2.overlays.insert(
                        0,
                        Rc::new(super::broadcast_reuse_peer::BroadcastReuserClass),
                    );
                    *reuser_has_been_inserted = true;
                }
            }
            (IsItself, IsItself) => {
                info!("Special mode, expection from usual one-stdio rule. Acting like `cat(1)`");
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
            && (self.s2.addrtype.get_name() == "WriteFileClass"
                || self.s2.addrtype.get_name() == "AppendFileClass")
            && self.s2.reuser_count() == 0
        {
            info!("Auto-inserting the reuser");
            self.s2
                .overlays
                .push(Rc::new(super::primitive_reuse_peer::ReuserClass));
        };
        Ok(())
    }
    fn l_exec(&mut self, on_warning: &OnWarning) -> Result<()> {
        if self.s1.addrtype.get_name() == "ExecClass" && self.s2.addrtype.get_name() == "ExecClass"
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
    
    fn l_socks5_c(s: &mut SpecifierStack, opts: &mut Options, on_warning: &OnWarning) -> Result<()> {
        let url = format!("ws://{}",s.addr);
        
        // Overwrite WsClientClass
        s.addrtype = Rc::new(super::net_peer::TcpConnectClass);
        
        match opts.auto_socks5.unwrap() {
            SocketAddr::V4(sa4) => {
                s.addr = format!("{}:{}", sa4.ip(), sa4.port());
            },
            SocketAddr::V6(sa6) => {
                s.addr = format!("[{}]:{}", sa6.ip(), sa6.port());
            },
        }
        
        
        use self::hyper::Url;
        use self::url::Host;
        let u = Url::parse(&url)?;
        
        if !u.has_host() {
            Err("WebSocket URL has not host")?;
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
        
        
        
        if opts.ws_c_uri != "ws://0.0.0.0/" {
            on_warning("Looks like you've overridden ws-c-uri. We are overwriting it for --socks5 option.");
        }
        
        opts.ws_c_uri = url;
        
        
        s.overlays.push(Rc::new(super::ws_client_peer::WsConnectClass));
        s.overlays.push(Rc::new(super::proxy_peer::SocksProxyClass));
        
        Ok(())
    }
    
    fn l_socks5(&mut self, on_warning: &OnWarning) -> Result<()> {
        if self.opts.socks_destination.is_some() ^ (self.contains_class("SocksProxyClass") || self.contains_class("SocksBindClass")) {
            on_warning("--socks5-destination option and socks5-connect: overlay should go together");
        }
        
        if self.opts.auto_socks5.is_some() {
            if !((self.s1.addrtype.get_name() == "WsClientClass" 
                 || self.s1.addrtype.get_name() == "WsClientSecureClass")
                ^ 
                (self.s2.addrtype.get_name() == "WsClientClass" 
                || self.s2.addrtype.get_name() == "WsClientSecureClass")) {
                
                
                Err("User-friendly --socks5 option supports socksifying exactly one non-raw websocket client connection. You are using two or none.")?;
            }
            
            
            if self.s1.addrtype.get_name() == "WsClientClass" {
                WebsocatConfiguration2::l_socks5_c(&mut self.s1, &mut self.opts, on_warning)?;
            }
            if self.s1.addrtype.get_name() == "WsClientSecureClass" {
                Err("Unfortunately, socksifying wss:// connection is not yet supported. Use ws-c:cmd: workaround.")?
            }
            if self.s2.addrtype.get_name() == "WsClientClass" {
                WebsocatConfiguration2::l_socks5_c(&mut self.s2, &mut self.opts, on_warning)?;
            }
            if self.s2.addrtype.get_name() == "WsClientSecureClass" {
                Err("Unfortunately, socksifying wss:// connection is not yet supported. Use ws-c:cmd: workaround.")?
            }
        }
        Ok(())
    }

    pub fn lint_and_fixup(&mut self, on_warning: OnWarning) -> Result<()> {
        let multiconnect = !self.opts.oneshot && self.s1.is_multiconnect();
        let mut reuser_has_been_inserted = false;

        self.l_stdio(multiconnect, &mut reuser_has_been_inserted)?;
        self.l_reuser(reuser_has_been_inserted)?;
        self.l_linemode()?;
        self.l_listener_on_the_right(&on_warning)?;
        self.l_reuser_for_append(multiconnect)?;
        self.l_exec(&on_warning)?;
        self.l_uri_staticfiles(&on_warning)?;
        self.l_environ(&on_warning)?;
        self.l_closebug(&on_warning)?;
        self.l_socks5(&on_warning)?;

        // TODO: UDP connect oneshot mode
        // TODO: tests for the linter
        Ok(())
    }
}
