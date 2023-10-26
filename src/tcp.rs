use std::net::SocketAddr;

use rhai::{Engine, Dynamic, FnPtr, EvalAltResult};

use crate::{types::{Handle, Task, StreamSocket, TaskHandleExt, run_task}, scenario::callback};

fn connect_tcp(opts: Dynamic, continuation: FnPtr) -> Result<Handle<Task>,Box<EvalAltResult>> {
    #[derive(serde::Deserialize)]
    struct TcpOpts {
        addr: SocketAddr,
    }
    let opts : TcpOpts = rhai::serde::from_dynamic(&opts)?;

    Ok(async move {
        let t = tokio::net::TcpStream::connect(opts.addr).await;
        let t = match t {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Error: {e}");
                return;
            }
        };
        let (r,w) = t.into_split();
        let (r,w) = (Box::pin(r), Box::pin(w));

        eprintln!("tcp read={r:p} write={w:p}");

        let h = StreamSocket {
            read: Some(r),
            write: Some(w),
            close: None,
        }.wrap();

        


        match callback(continuation, (h,)) {
            Ok(h) => run_task(h).await,
            Err(e) => eprintln!("Error: {e}"),
        };
    }.wrap())
}


pub fn register(engine: &mut Engine) {
    engine.register_fn("connect_tcp", connect_tcp);
}
