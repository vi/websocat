use rhai::{Dynamic, Engine};
use tracing::{debug, debug_span, error, field, Instrument};

use crate::{
    debugfluff::PtrDbg,
    types::{Handle, StreamRead, StreamSocket, StreamWrite, Task},
    utils::{run_task, HandleExt, TaskHandleExt},
};

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
    async move {}.wrap()
}

fn sleep_ms(ms: i64) -> Handle<Task> {
    async move { tokio::time::sleep(std::time::Duration::from_millis(ms as u64)).await }.wrap()
}

fn sequential(tasks: Vec<Dynamic>) -> Handle<Task> {
    async move {
        for t in tasks {
            let Some(t): Option<Handle<Task>> = t.try_cast() else {
                eprintln!("Not a task in a list of tasks");
                continue;
            };
            run_task(t).await;
        }
    }
    .wrap()
}

fn parallel(tasks: Vec<Dynamic>) -> Handle<Task> {
    async move {
        let mut waitees = Vec::with_capacity(tasks.len());
        for t in tasks {
            let Some(t): Option<Handle<Task>> = t.try_cast() else {
                eprintln!("Not a task in a list of tasks");
                continue;
            };
            waitees.push(tokio::spawn(run_task(t)));
        }
        for w in waitees {
            let _ = w.await;
        }
    }
    .wrap()
}

fn spawn_task(task: Handle<Task>) {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "spawn", t = field::Empty);
    if let Some(x) = task.lock().unwrap().as_ref() {
        span.record("t", tracing::field::debug(PtrDbg(&**x)));
        debug!(parent: &span, "Spawning");
    } else {
        error!("Attempt to spawn a null/taken task");
    }
    tokio::spawn(
        async move {
            run_task(task).await;
            debug!("Finished");
        }
        .instrument(span),
    );
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("take_read_part", take_read_part);
    engine.register_fn("take_write_part", take_write_part);
    engine.register_fn("dummy_task", dummytask);
    engine.register_fn("sleep_ms", sleep_ms);
    engine.register_fn("sequential", sequential);
    engine.register_fn("parallel", parallel);
    engine.register_fn("spawn_task", spawn_task);
}
