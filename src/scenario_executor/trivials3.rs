use rhai::{Engine, FnPtr, NativeCallContext};
use tracing::debug;

use crate::scenario_executor::{scenario::callback_and_continue, types::Handle, utils1::HandleExt};

use super::{scenario::ScenarioAccess, types::{Hangup, Task}, utils1::{RhResult, SimpleErr}};

pub struct TriggerableEventTrigger {
    tx: tokio::sync::oneshot::Sender<()>,
}

pub struct TriggerableEvent {
    waiter_part: Option<Hangup>,
    trigger_part: Option<TriggerableEventTrigger>,
}

//@ Create new one-time synchromisation object that allows to trigger a hangup event explicitly from Rhai code.
fn triggerable_event_create() -> Handle<TriggerableEvent> {
    let (tx,rx) = tokio::sync::oneshot::channel();
    let signal = TriggerableEvent {
        waiter_part: Some(Box::pin(async move {let _ = rx.await;})),
        trigger_part: Some(TriggerableEventTrigger { tx }),
    };
    Some(signal).wrap()
}

//@ Take the waitable part (Hangup) from an object created by `triggerable_event_create`
fn triggerable_event_take_hangup(
    ctx: NativeCallContext,
    h: &mut Handle<TriggerableEvent>,
) -> RhResult<Handle<Hangup>> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        Ok(s.waiter_part.take().wrap())
    } else {
        Err(ctx.err("TriggerableEvent's hangup part is already taken"))
    }
}

//@ Take the activatable part from an object created by `triggerable_event_create`
fn triggerable_event_take_trigger(
    ctx: NativeCallContext,
    h: &mut Handle<TriggerableEvent>,
) -> RhResult<Handle<TriggerableEventTrigger>> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        Ok(s.trigger_part.take().wrap())
    } else {
        Err(ctx.err("TriggerableEvent's trigger part is already taken"))
    }
}

//@ Trigger the activatable part from an object created by `triggerable_event_create`.
//@ This should cause a hangup even on the associated Hangup object.
fn triggerable_event_fire(
    ctx: NativeCallContext,
    h: &mut Handle<TriggerableEventTrigger>,
) -> RhResult<()> {
    if let Some(s) = h.lock().unwrap().take() {
        let _ = s.tx.send(());
        Ok(())
    } else {
        Err(ctx.err("TriggerableEventTrigger is already used"))
    }
}

//@ Create a Task that runs specified Rhai code when scheduled.
fn task_wrap(
    ctx: NativeCallContext,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let the_scenario = ctx.get_scenario()?;
    
    let t : Task = Box::pin(async move {
        debug!("task_wrap");
        callback_and_continue::<()>(the_scenario, continuation, ()).await;
        Ok(())
    });
    Ok(Some(t).wrap())
}


pub fn register(engine: &mut Engine) {
    engine.register_fn("triggerable_event_create", triggerable_event_create);
    engine.register_fn("take_hangup", triggerable_event_take_hangup);
    engine.register_fn("take_trigger", triggerable_event_take_trigger);
    engine.register_fn("fire", triggerable_event_fire);
    engine.register_fn("task_wrap", task_wrap);
}
