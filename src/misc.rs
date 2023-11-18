use rhai::Engine;

use crate::types::{Handle, StreamRead, StreamSocket, StreamWrite};

fn create_stdio() -> Handle<StreamSocket> {
    StreamSocket {
        read: Some(StreamRead {
            reader: Box::pin(tokio::io::stdin()),
            prefix: Default::default(),
        }),
        write: Some(StreamWrite {
            writer: Box::pin(tokio::io::stdout()),
        }),
        close: None,
    }
    .wrap()
}
pub fn register(engine: &mut Engine) {
    engine.register_fn("create_stdio", create_stdio);
}
