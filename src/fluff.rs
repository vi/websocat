use bytes::BytesMut;
use object_pool::Pool;
use rhai::Engine;
use std::sync::{Arc, Mutex};

use crate::types::{DatagramSink, Handle, DatagramStream, Buffer, BufferPool};


fn trivial_pkts() -> Handle<DatagramStream> {
    //let b : Buffer = Box::new(&b"qqq\n"[..]);
    let pool = Arc::new(Pool::new(1, ||BytesMut::new()));
    let mut b = pool.pull(||BytesMut::new()).detach().1;
    b.clear();
    b.resize(4, 0);
    b.copy_from_slice(b"q2q\n");
    let mut buf = Buffer::new();
    buf.data.push(b);
    Arc::new(Mutex::new(Some(DatagramStream {
        src: Box::pin(futures::stream::iter([buf])),
        pool,
    })))
}



fn display_pkts() -> Handle<DatagramSink> {
    let pool : Handle<BufferPool> = Arc::new(Mutex::new(None));
    let pool_ = pool.clone();
    let snk = Box::pin(futures::sink::unfold((), move |_:(), mut item: Buffer| {
        let pool = pool_.clone();
        async move {
            eprintln!("QQQ {}", std::str::from_utf8(&item.data[0][..]).unwrap());
            if let Ok(a) = pool.try_lock() {
                if let Some(ref b) = *a {
                    item.recycle(b);
                }
            }
            Ok(())
        }
    }));
    Arc::new(Mutex::new(Some(DatagramSink { snk, pool })))
}


pub fn register(engine: &mut Engine) {
    engine.register_fn("trivial_pkts", trivial_pkts);
    engine.register_fn("display_pkts", display_pkts);
}
