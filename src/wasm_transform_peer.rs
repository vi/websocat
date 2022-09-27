use futures::future::ok;
use wasmtime::{Module, Linker, Memory, Caller, TypedFunc};

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::AtomicU32;
use std::sync::{Mutex, Arc};

use crate::peer_strerr;

use super::{BoxedNewPeerFuture, Peer};
use super::{ConstructParams, PeerConstructor, Specifier};

use std::io::Read;
use tokio_io::AsyncRead;

use std::io::Error as IoError;



#[derive(Default)]
struct Env {
    store: wasmtime::Store::<()>,
    modules: HashMap<String, wasmtime::Instance>,
}

static ENV: Mutex<Option<Env>> = Mutex::new(None);
static COUNTER: AtomicU32 = AtomicU32::new(0);

#[derive(Clone)]
pub struct Handle {
    mem: Memory,
    malloc: TypedFunc<u32, u32>,
    free: TypedFunc<u32, ()>,
    transform: TypedFunc<(u32, u32, u32, u32, u32), u32>,
}
impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handle").finish()
    }
}

macro_rules! declare_wasm_transform_class {
    ($PI:ident, $C:ident, $opt:ident, $prefix:literal, $help:literal $(,)?) => {
        #[derive(Debug)]
        pub struct $PI<T: Specifier>(pub T);
        impl<T: Specifier> Specifier for $PI<T> {
            fn construct(&self, cp: ConstructParams) -> PeerConstructor {
                let inner = self.0.construct(cp.clone());
                inner
                    .map(move |p, _| transform_peer(p, cp.program_options.$opt.clone()))
            }
            specifier_boilerplate!(has_subspec noglobalstate);
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

declare_wasm_transform_class!(
    WasmTransformA,
    WasmTransformAClass,
    wasm_transform_a,
    "wasm_plugin_transform_a:",
    r#"
[A] Custom overlay that transforms data being read from inner specifier by calling a function from a loaded WebAssembly plugin.
Writes go through unmodified. Multiple distinct transforms may be active at the same time, see "_b", "_c" and "_d" postfixes

Example:

    emcc --no-entry -sERROR_ON_UNDEFINED_SYMBOLS=0  -s STANDALONE_WASM  transform_plugins/sample.c  \
            -Wl,--export=websocat_transform,--export=malloc,--export=free -o transform_plugins/sample.wasm
    
    websocat -Eb ws-l:127.0.0.1:1234 wasm_plugin_transform_a:mirror: --wasm-plugin-a=transform_plugins/sample.wasm
"#
);

declare_wasm_transform_class!(
    WasmTransformB,
    WasmTransformBClass,
    wasm_transform_b,
    "wasm_plugin_transform_b:",
    "[A] Same as `wasm_plugin_transform_a`, but for other plugin slot.",
);
declare_wasm_transform_class!(
    WasmTransformC,
    WasmTransformCClass,
    wasm_transform_c,
    "wasm_plugin_transform_c:",
    "[A] Same as `wasm_plugin_transform_a`, but for other plugin slot.",
);
declare_wasm_transform_class!(
    WasmTransformD,
    WasmTransformDClass,
    wasm_transform_d,
    "wasm_plugin_transform_d:",
    "[A] Same as `wasm_plugin_transform_a`, but for other plugin slot.",
);


pub fn load_symbol(spec: &str) -> crate::Result<Handle> {
    let mut env = ENV.lock().unwrap();
    let env = env.get_or_insert_with(Default::default);
    
    let (libname, symname) =
    if let Some((before, after)) = spec.split_once('@') {
        (after, before)
    } else {
        (spec, "websocat_transform")
    };
    
    let instance = match env.modules.entry(libname.to_owned()) {
        std::collections::hash_map::Entry::Occupied(x) => x.into_mut(),
        std::collections::hash_map::Entry::Vacant(x) => {
            let module = if libname.starts_with('!') {
                info!("Loading pre-built wasm module {}", &libname[1..]);
                unsafe { Module::deserialize_file(env.store.engine(), &libname[1..])? }
            } else {
                #[cfg(feature="wasm_compiler")] {
                    info!("Compiling wasm module {}", libname);
                    Module::from_file(env.store.engine(), libname)?
                }
                #[cfg(not(feature="wasm_compiler"))] {
                    return Err("Compiling wasm modules is not enabled in this Websocat build. Pre-compile them using `wasmtime compile`, then specify as `!myfilename.cwasm`")?;
                }
            };
            
            let mut linker = Linker::<()>::new(env.store.engine());
            let mem_cell = Arc::new(Mutex::new(None::<Memory>));
            let mem_cell2 = mem_cell.clone();
            linker.func_wrap("env", "websocat_log", move |c: Caller<()>, severity: i32, buffer: u32, mut len: u32| {
                if len > 4096 {
                    len = 4096;
                }
                let mut buf = vec![0u8; len as usize]; 
                mem_cell2.lock().unwrap().unwrap().read(c, buffer as usize, &mut buf[..]).unwrap();

                let level = match severity {
                    x if x<=1 => log::Level::Error,
                    2 => log::Level::Warn,
                    3 => log::Level::Info,
                    4 => log::Level::Debug,
                    _ => log::Level::Trace,
                };
                log::log!(level, "{}", std::string::String::from_utf8_lossy(&buf[..]));
            })?;
        
            let instance = linker.instantiate(&mut env.store, &module).unwrap();
            let mem = instance.get_memory(&mut env.store, "memory").ok_or_else(||"no memory")?;
            *mem_cell.lock().unwrap() = Some(mem);
            debug!("Wasm module loaded successfully");
            x.insert(instance)
        }
    };

    debug!("Instantiating {}", spec);
    let mem = instance.get_memory(&mut env.store, "memory").ok_or_else(||"no memory")?;
    let transform = instance.get_typed_func::<(u32, u32, u32, u32, u32), u32, _>(&mut env.store, symname)?;
    let malloc = instance.get_typed_func::<u32, u32, _>(&mut env.store, "malloc")?;
    let free = instance.get_typed_func::<u32, (), _>(&mut env.store, "free")?;
    debug!("Instantiated");
    
    let h = Handle {
        mem,
        malloc,
        free,
        transform,
    };

    Ok(h)
}

pub fn transform_peer(inner_peer: Peer, s: Option<Handle>) -> BoxedNewPeerFuture {
    if s.is_none() {
        return peer_strerr("Symbol for wasm_plugin_transform_... is not specified");
    }

    let conn_seqn = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let s = s.unwrap();


    let mut env = ENV.lock().unwrap();
    let env = env.as_mut().unwrap();
    
    if let Err(_e) = s.transform.call(&mut env.store, (0, 0, 0, conn_seqn, 0)) {
        return peer_strerr("Failed to call symbol from wasm module");
    }

    let filtered_r = TransformPeer {
        inner: inner_peer.0,
        sym: s,
        seqn: 1,
        conn_seqn,
        buf: 0,
        buf_cap: 0,
    };
    let thepeer = Peer::new(filtered_r, inner_peer.1, inner_peer.2);


    Box::new(ok(thepeer)) as BoxedNewPeerFuture
}

struct TransformPeer {
    inner: Box<dyn AsyncRead>,
    sym: Handle,
    seqn: u32,
    conn_seqn: u32,
    buf: u32,
    buf_cap: u32,
}

impl TransformPeer {
    fn transform(&mut self, b: &mut [u8], l:u32, mut n:u32) -> crate::Result<u32> {

        let mut env = ENV.lock().unwrap();
        let env = env.as_mut().unwrap();

        if self.buf == 0 {
            self.buf = self.sym.malloc.call(&mut env.store, l)?;
            self.buf_cap = l;

            if self.buf == 0 {
                return Err("Allocation failed inside wasm module")?;
            }
        }

        if self.buf_cap < n {
            warn!("Trimming message to be processed in wasm due to larger read than expected");
            n = self.buf_cap;
        }
        self.sym.mem.write(&mut env.store, self.buf as usize, &b[0..(n as usize)])?;
        let mut k = self.sym.transform.call(&mut env.store, (self.buf, n, self.buf_cap, self.conn_seqn, self.seqn))?;
        if k > self.buf_cap || k > l {
            warn!("Invalid return value from wasm transform function");
            k = self.buf_cap.min(l);
        }
        self.sym.mem.read(&mut env.store, self.buf as usize, &mut b[0..(k as usize)])?;

        Ok(k)
    }
}

impl Read for TransformPeer {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        let mut l = b.len();

        if l > i32::MAX as usize {
            l = i32::MAX as usize;
        }

        let n = match self.inner.read(&mut b[..l]) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };

        if n == 0 {
            return Ok(0);
        }

        let k = match self.transform(b, l as u32, n as u32) {
            Ok(k) => k,
            Err(e) => return Err(crate::util::simple_err(format!("{}", e))),
        };
        self.seqn += 1;

        Ok(k as usize)
    }
}
impl AsyncRead for TransformPeer {}
impl Drop for TransformPeer {
    fn drop(&mut self) {
        let mut env = ENV.lock().unwrap();
        let env = env.as_mut().unwrap();
        let _ = self.sym.transform.call(&mut env.store, (0, 0, 0, self.conn_seqn, self.seqn));
        if self.buf != 0 {
            let _ = self.sym.free.call(&mut env.store, self.buf);
            self.buf = 0;
        }
    }
}
