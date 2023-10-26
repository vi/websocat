use rhai::{Engine, Dynamic};

use crate::types::{Handle, StreamSocket, StreamRead, StreamWrite, Task, TaskHandleExt, HandleExt, run_task};

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

fn sequential(tasks: Vec<Dynamic> ) -> Handle<Task> {
    async move {
        for t in tasks {
            let Some(t) : Option<Handle<Task>> = t.try_cast() else {
                eprintln!("Not a task in a list of tasks");
                continue;
            };
            run_task(t).await;
        }
    }.wrap()
}

fn parallel(tasks: Vec<Dynamic> ) -> Handle<Task> {
    async move {
        let mut waitees = Vec::with_capacity(tasks.len());
        for t in tasks {
            let Some(t) : Option<Handle<Task>> = t.try_cast() else {
                eprintln!("Not a task in a list of tasks");
                continue;
            };
            waitees.push(tokio::spawn(run_task(t)));
        }
        for w in waitees {
            let _ = w.await;
        }
    }.wrap()
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("take_read_part", take_read_part);
    engine.register_fn("take_write_part", take_write_part);
    engine.register_fn("dummy_task", dummytask);
    engine.register_fn("sequential", sequential);
    engine.register_fn("parallel", parallel);
}
