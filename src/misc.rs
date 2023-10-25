use rhai::Engine;

use crate::types::{Handle, StreamSocket};

fn create_stdio() -> Handle<StreamSocket> {
    StreamSocket {
        read: Some(Box::pin(tokio::io::stdin())),
        write: Some(Box::pin(tokio::io::stdout())),
        close: None,
    }.wrap()
}
pub fn register(engine: &mut Engine) {
    engine.register_fn("create_stdio", create_stdio);
}
