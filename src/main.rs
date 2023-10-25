use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex}, net::SocketAddr,
};

use bytes::BytesMut;
use futures::{stream::Stream, sink::Sink, StreamExt};
use object_pool::Pool;
use rhai::{Engine, Dynamic, EvalAltResult, Map, Position, FnPtr, AST, NativeCallContext};
use tokio::{io::{AsyncRead, AsyncWrite}, net::TcpStream};

type Handle<T> = Arc<Mutex<Option<T>>>;

pub type StreamRead = Option<Pin<Box<dyn AsyncRead + Send>>>;
pub type StreamWrite = Option<Pin<Box<dyn AsyncWrite + Send>>>;
pub type Hangup = Option<Pin<Box<dyn Future<Output = ()> + Send>>>;
pub type Task = Option<Pin<Box<dyn Future<Output = ()> + Send>>>;
pub struct StreamSocket {
    pub read: StreamRead,
    pub write: StreamWrite,
    pub close: Hangup,
}
pub type StreamReadHandle = Arc<Mutex<StreamRead>>;
pub type StreamWriteHandle = Arc<Mutex<StreamWrite>>;
pub type HangpHandle = Arc<Mutex<Hangup>>;
pub type TaskHandle = Arc<Mutex<Task>>;
pub type StreamSocketHandle = Arc<Mutex<StreamSocket>>;

fn create_stdio() -> StreamSocketHandle {
    Arc::new(Mutex::new(StreamSocket {
        read: Some(Box::pin(tokio::io::stdin())),
        write: Some(Box::pin(tokio::io::stdout())),
        close: None,
    }))
}

fn take_read_part(h: StreamSocketHandle) -> StreamReadHandle {
    if let Some(hh) = h.lock().unwrap().read.take() {
        Arc::new(Mutex::new(Some(hh)))
    } else {
        Arc::new(Mutex::new(None))
    }
}
fn take_write_part(h: StreamSocketHandle) -> StreamWriteHandle {
    if let Some(hh) = h.lock().unwrap().write.take() {
        Arc::new(Mutex::new(Some(hh)))
    } else {
        Arc::new(Mutex::new(None))
    }
}
fn copydata(from: StreamReadHandle, to: StreamWriteHandle) -> TaskHandle {
    Arc::new(Mutex::new(Some(Box::pin(async move {
        let (f, t) = (from.lock().unwrap().take(), to.lock().unwrap().take());
        if let (Some(mut r), Some(mut w)) = (f, t) {
            match tokio::io::copy(&mut r, &mut w).await {
                Ok(x) => eprintln!("Copied {x} bytes"),
                Err(e) => eprintln!("Error from copydata: {e}"),
            }
        } else {
            eprintln!("Nothing to copydata");
        }
    }))))
}

/*fn copybidir(s1: StreamSocketHandle, s2: StreamSocketHandle) -> TaskHandle {
    Arc::new(Mutex::new(Some(Box::pin(async move {
        let (f, t) = (from.lock().unwrap().take(), to.lock().unwrap().take());
        if let (Some(mut r), Some(mut w)) = (f, t) {
            match tokio::io::copy(&mut r, &mut w).await {
                Ok(x) => eprintln!("Copied {x} bytes"),
                Err(e) => eprintln!("Error from copydata: {e}"),
            }
        } else {
            eprintln!("Nothing to copydata");
        }
    }))))
}*/
fn dummytask() -> TaskHandle {
    Arc::new(Mutex::new(Some(Box::pin(async move {
        
    }))))
}

pub struct Buffer {
    data: BytesMut,
}
impl Buffer {
    pub fn new() -> Buffer {
        Buffer { data: BytesMut::new() }
    }
    pub fn clear(&mut self) {
        self.data.clear();
    }
}

pub struct DatagramStream {
    pub src: Pin<Box<dyn Stream<Item = Buffer> + Send>>,
    pub pool: Arc<Pool<Buffer>>,
}

type DatagramStreamHandle = Arc<Mutex<Option<DatagramStream>>>;
type PoolHandle = Handle<Arc<Pool<Buffer>>>;

fn trivial_pkts() -> DatagramStreamHandle {
    //let b : Buffer = Box::new(&b"qqq\n"[..]);
    let pool = Arc::new(Pool::new(1024, ||Buffer::new()));
    let mut b = pool.pull(||Buffer::new()).detach().1;
    b.clear();
    b.data.resize(4, 0);
    b.data.copy_from_slice(b"q2q\n");
    Arc::new(Mutex::new(Some(DatagramStream {
        src: Box::pin(futures::stream::iter([b])),
        pool,
    })))
}


pub struct DatagramSink {
    pub snk: Pin<Box<dyn Sink<Buffer, Error = ()> + Send>>,
    pub pool: PoolHandle,
}
type DatagramSinkHandle = Handle<DatagramSink>;

fn display_pkts() -> DatagramSinkHandle {
    let pool : PoolHandle = Arc::new(Mutex::new(None));
    let pool_ = pool.clone();
    let snk = Box::pin(futures::sink::unfold((), move |_:(), item: Buffer| {
        let pool = pool_.clone();
        async move {
            eprintln!("QQQ {}", std::str::from_utf8(&item.data[..]).unwrap());
            if let Ok(a) = pool.try_lock() {
                if let Some(ref b) = *a {
                    b.attach(item);
                }
            }
            Ok(())
        }
    }));
    Arc::new(Mutex::new(Some(DatagramSink { snk, pool })))
}


fn copy_packets(from: DatagramStreamHandle, to: DatagramSinkHandle) -> TaskHandle {
    Arc::new(Mutex::new(Some(Box::pin(async move {
        let (f, t) = (from.lock().unwrap().take(), to.lock().unwrap().take());
        if let (Some(r), Some(w)) = (f, t) {
            *w.pool.lock().unwrap() = Some(r.pool.clone());
            match r.src.map(|x|Ok::<Buffer,()>(x)).forward(w.snk).await {
                Ok(()) => eprintln!("Finished forwarding"),
                Err(()) => eprintln!("Error from copy_packets"),
            }
        } else {
            eprintln!("Nothing to copydata");
        }
    }))))
}


fn connect_tcp(ast: &'static AST, ctx: NativeCallContext<'_>, opts: Dynamic, continuation: FnPtr) -> Result<TaskHandle,Box<EvalAltResult>> {
    #[derive(serde::Deserialize)]
    struct TcpOpts {
        addr: SocketAddr,
    }
    let opts : TcpOpts = rhai::serde::from_dynamic(&opts)?;

    let e = ctx.engine();
    let e : &'static Engine = unsafe { std::mem::transmute(e) };

    Ok(Arc::new(Mutex::new(Some(Box::pin(async move {
        let t = tokio::net::TcpStream::connect(opts.addr).await;
        let t = match t {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Error: {e}");
                return;
            }
        };
        let (r,w) = t.into_split();
    
        let h = Arc::new(Mutex::new(StreamSocket {
            read: Some(Box::pin(r)),
            write: Some(Box::pin(w)),
            close: None,
        }));


        let t : TaskHandle = continuation.call(e, ast, (h,)).unwrap();
        let t : Task = t.lock().unwrap().take();
        if let Some(t) = t {
            t.await;
        } else {
            eprintln!("No task requested");
        }
    })))))
}


#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let f = std::fs::read(std::env::args().nth(1).unwrap())?;

    let mut engine = Engine::RAW;
    let ast = engine.compile(std::str::from_utf8(&f[..])?)?;
    let ast : &'static AST = Box::leak(Box::new(ast));

    //let engine_h : Handle<Engine> = Arc::new(Mutex::new(None));

    engine.register_fn("create_stdio", create_stdio);
    engine.register_fn("take_read_part", take_read_part);
    engine.register_fn("take_write_part", take_write_part);
    engine.register_fn("copydata", copydata);

    engine.register_fn("trivial_pkts", trivial_pkts);
    engine.register_fn("display_pkts", display_pkts);
    engine.register_fn("copy_packets", copy_packets);
    engine.register_fn("dummy_task", dummytask);

    let conntcp = move |ctx: NativeCallContext, opts: Dynamic, continuation: FnPtr| -> Result<TaskHandle,Box<EvalAltResult>> {
        connect_tcp(ast, ctx, opts, continuation)
    };
    engine.register_fn("connect_tcp", conntcp);
    

    let task: TaskHandle = engine.eval_ast(&ast)?;

    if let Some(t) = task.lock().unwrap().take() {
        t.await;
    } else {
        eprintln!("No task requested");
    }

    Ok(())
}
