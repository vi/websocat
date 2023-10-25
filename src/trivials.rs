use rhai::Engine;

use crate::types::{Handle, StreamSocket, StreamRead, StreamWrite, Task, TaskHandleExt, HandleExt};

fn take_read_part(h: Handle<StreamSocket>) -> Handle<StreamRead> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        if let Some(hh) = s.read.take() {
            Some(hh).wrap()
        } else {
            None.wrap()
        }
    } else {
        None.wrap()
    }
}

fn take_write_part(h: Handle<StreamSocket>) -> Handle<StreamWrite> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        if let Some(hh) = s.write.take() {
            Some(hh).wrap()
        } else {
            None.wrap()
        }
    } else {
        None.wrap()
    }
}
fn dummytask() -> Handle<Task> {
    async move {
        
    }.wrap()
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("take_read_part", take_read_part);
    engine.register_fn("take_write_part", take_write_part);
    engine.register_fn("dummy_task", dummytask);
}
