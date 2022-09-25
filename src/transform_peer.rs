use futures::future::ok;

use std::rc::Rc;

use crate::peer_strerr;

use super::{BoxedNewPeerFuture, Peer};
use super::{ConstructParams, PeerConstructor, Specifier};

use std::io::Read;
use tokio_io::AsyncRead;

use std::io::Error as IoError;

pub type Counter = std::rc::Rc<std::cell::Cell<usize>>;

#[derive(Default, Clone)]
pub struct GlobalState(Counter);

macro_rules! declare_native_transform_class {
    ($PI:ident, $C:ident, $opt:ident, $prefix:literal, $help:literal $(,)?) => {
        #[derive(Debug)]
        pub struct $PI<T: Specifier>(pub T);
        impl<T: Specifier> Specifier for $PI<T> {
            fn construct(&self, cp: ConstructParams) -> PeerConstructor {
                let inner = self.0.construct(cp.clone());
                let g = cp.global(GlobalState::default);
                let seqn_counter = g.0.clone();
                drop(g);
                inner
                    .map(move |p, _| transform_peer(p, cp.program_options.$opt.clone(), seqn_counter.clone()))
            }
            specifier_boilerplate!(has_subspec globalstate);
            self_0_is_subspecifier!(proxy_is_multiconnect);
        }
        specifier_class!(
            name = $C,
            target = $PI,
            prefixes = [$prefix],
            arg_handling = subspec,
            overlay = true,
            MessageOriented,
            MulticonnectnessDependsOnInnerType,
            help = $help
        );
    };
}

declare_native_transform_class!(
    NativeTransformA,
    NativeTransformAClass,
    native_transform_a,
    "native_plugin_transform_a:",
    r#"
[A] Custom overlay that transforms data being read from inner specifier by calling a function from a loaded native plugin.
Writes go through unmodified. Multiple distinct transforms may be active at the same time, see "_b", "_c" and "_d" postfixes

Example:

    gcc -shared -fPIC transform_plugins/sample.c -o libmyplugin.so
    websocat -Eb ws-l:127.0.0.1:1234 native_plugin_transform_a:mirror: --native-plugin-a=./libmyplugin.so
    
"#
);
declare_native_transform_class!(
    NativeTransformB,
    NativeTransformBClass,
    native_transform_b,
    "native_plugin_transform_b:",
    "[A] Same as `native_plugin_transform_a`, but for other plugin slot.",
);
declare_native_transform_class!(
    NativeTransformC,
    NativeTransformCClass,
    native_transform_c,
    "native_plugin_transform_c:",
    "[A] Same as `native_plugin_transform_a`, but for other plugin slot.",
);
declare_native_transform_class!(
    NativeTransformD,
    NativeTransformDClass,
    native_transform_d,
    "native_plugin_transform_d:",
    "[A] Same as `native_plugin_transform_a`, but for other plugin slot.",
);

type F = unsafe extern "C" fn(
    buf: *mut std::ffi::c_uchar,
    len: usize,
    cap: usize,
    conn_n: usize,
    packet_n: usize,
) -> usize;
pub type Sym = libloading::Symbol<'static, F>;

pub fn load_symbol(spec: &str) -> crate::Result<Sym> {
    let (libname, symname) =
    if let Some((before, after)) = spec.split_once('@') {
        (after, before)
    } else {
        (spec, "websocat_transform")
    };
    let s = unsafe {
        let l = libloading::Library::new(libname)?;
        let l = Box::leak(Box::new(l));
        l.get(symname.as_bytes())?
    };
    Ok(s)
}

pub fn transform_peer(inner_peer: Peer, s: Option<Sym>, conn_seqn_counter: Counter) -> BoxedNewPeerFuture {
    if s.is_none() {
        return peer_strerr("Symbol for native_plugin_transform_... is not specified");
    }

    let s = s.unwrap();

    let conn_seqn = conn_seqn_counter.get();
    conn_seqn_counter.set(conn_seqn+1);

    unsafe { (s)(std::ptr::null_mut(), 0, 0, conn_seqn, 0) };

    let filtered_r = TransformPeer {
        inner: inner_peer.0,
        sym: s,
        seqn: 1,
        conn_seqn,
    };
    let thepeer = Peer::new(filtered_r, inner_peer.1, inner_peer.2);


    Box::new(ok(thepeer)) as BoxedNewPeerFuture
}
struct TransformPeer {
    inner: Box<dyn AsyncRead>,
    sym: Sym,
    seqn: usize,
    conn_seqn: usize,
}

impl Read for TransformPeer {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        let l = b.len();

        let n = match self.inner.read(&mut b[..l]) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };

        if n == 0 {
            return Ok(0);
        }

        let k = unsafe { (self.sym)(b.as_mut_ptr(), n, l, self.conn_seqn, self.seqn) };
        self.seqn += 1;

        Ok(k)
    }
}
impl AsyncRead for TransformPeer {}
impl Drop for TransformPeer {
    fn drop(&mut self) {
        unsafe { (self.sym)(std::ptr::null_mut(), 0, 0, self.conn_seqn, self.seqn) };
    }
}


#[no_mangle]
pub extern "C" fn websocat_log(severity: libc::c_int, data: *const u8, len: usize) {
    let s = unsafe { std::slice::from_raw_parts(data, len) };
    let s = unsafe { std::str::from_utf8_unchecked(s) };
    let level = match severity {
        x if x<=1 => log::Level::Error,
        2 => log::Level::Warn,
        3 => log::Level::Info,
        4 => log::Level::Debug,
        _ => log::Level::Trace,
    };
    log::log!(level, "{}", s);
}
