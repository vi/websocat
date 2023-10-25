use std::net::SocketAddr;

use rhai::{Engine, Dynamic, FnPtr, EvalAltResult};

use crate::{types::{Handle, Task, StreamSocket, TaskHandleExt}, THE_ENGINE, THE_AST};

fn connect_tcp(opts: Dynamic, continuation: FnPtr) -> Result<Handle<Task>,Box<EvalAltResult>> {
    #[derive(serde::Deserialize)]
    struct TcpOpts {
        addr: SocketAddr,
    }
    let opts : TcpOpts = rhai::serde::from_dynamic(&opts)?;

    let engine = THE_ENGINE.lock().unwrap().as_ref().unwrap().clone();
    let ast = THE_AST.lock().unwrap().as_ref().unwrap().clone();

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
    
        let h = StreamSocket {
            read: Some(Box::pin(r)),
            write: Some(Box::pin(w)),
            close: None,
        }.wrap();


        let t : Handle<Task> = continuation.call(&*engine, &*ast, (h,)).unwrap();
        let t : Option<Task> = t.lock().unwrap().take();
        if let Some(t) = t {
            t.await;
        } else {
            eprintln!("No task requested");
        }
    }.wrap())
}


pub fn register(engine: &mut Engine) {
    engine.register_fn("connect_tcp", connect_tcp);
}
