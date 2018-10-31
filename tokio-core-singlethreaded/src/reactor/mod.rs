//! The core reactor driving all I/O
//!
//! This module contains the `Core` type which is the reactor for all I/O
//! happening in `tokio-core`. This reactor (or event loop) is used to run
//! futures, schedule tasks, issue I/O requests, etc.

use std::cell::RefCell;
use std::fmt;
use std::io;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, ATOMIC_USIZE_INIT, Ordering};
use std::time::{Instant, Duration};

use tokio;
use tokio::executor::current_thread::{CurrentThread, TaskExecutor};
use tokio_executor;
use tokio_executor::park::{Park, Unpark, ParkThread, UnparkThread};
use tokio_timer::timer::{self, Timer};

use futures::{Future, IntoFuture, Async};
use futures::future::{self, Executor, ExecuteError};
use futures::executor::{self, Spawn, Notify};
use futures::sync::mpsc;
use mio;

mod poll_evented;
mod poll_evented2;
mod timeout;
mod interval;
pub use self::poll_evented::PollEvented;
pub(crate) use self::poll_evented2::PollEvented as PollEvented2;
pub use self::timeout::Timeout;
pub use self::interval::Interval;

static NEXT_LOOP_ID: AtomicUsize = ATOMIC_USIZE_INIT;
scoped_thread_local!(static CURRENT_LOOP: Core);

/// An event loop.
///
/// The event loop is the main source of blocking in an application which drives
/// all other I/O events and notifications happening. Each event loop can have
/// multiple handles pointing to it, each of which can then be used to create
/// various I/O objects to interact with the event loop in interesting ways.
// TODO: expand this
pub struct Core {
    /// Uniquely identifies the reactor
    id: usize,

    /// Handle to the Tokio runtime
    rt: tokio::runtime::Runtime,

    /// Executes tasks
    executor: RefCell<CurrentThread<Timer<ParkThread>>>,

    /// Timer handle
    timer_handle: timer::Handle,

    /// Wakes up the thread when the `run` future is notified
    notify_future: Arc<MyNotify>,

    /// Wakes up the thread when a message is posted to `rx`
    notify_rx: Arc<MyNotify>,

    /// Send messages across threads to the core
    tx: mpsc::UnboundedSender<Message>,

    /// Receive messages
    rx: RefCell<Spawn<mpsc::UnboundedReceiver<Message>>>,

    // Shared inner state
    inner: Rc<RefCell<Inner>>,
}

struct Inner {
    // Tasks that need to be spawned onto the executor.
    pending_spawn: Vec<Box<Future<Item = (), Error = ()>>>,
}

/// An unique ID for a Core
///
/// An ID by which different cores may be distinguished. Can be compared and used as an index in
/// a `HashMap`.
///
/// The ID is globally unique and never reused.
#[derive(Clone,Copy,Eq,PartialEq,Hash,Debug)]
pub struct CoreId(usize);

/// Handle to an event loop, used to construct I/O objects, send messages, and
/// otherwise interact indirectly with the event loop itself.
///
/// Handles can be cloned, and when cloned they will still refer to the
/// same underlying event loop.
#[derive(Clone)]
pub struct Remote {
    id: usize,
    tx: mpsc::UnboundedSender<Message>,
    new_handle: tokio::reactor::Handle,
    timer_handle: timer::Handle,
}

/// A non-sendable handle to an event loop, useful for manufacturing instances
/// of `LoopData`.
#[derive(Clone)]
pub struct Handle {
    remote: Remote,
    inner: Weak<RefCell<Inner>>,
    thread_pool: ::tokio::runtime::TaskExecutor,
}

enum Message {
    Run(Box<FnBox>),
}

// ===== impl Core =====

impl Core {
    /// Creates a new event loop, returning any error that happened during the
    /// creation.
    pub fn new() -> io::Result<Core> {
        // Create a new parker
        let timer = Timer::new(ParkThread::new());

        // Create notifiers
        let notify_future = Arc::new(MyNotify::new(timer.unpark()));
        let notify_rx = Arc::new(MyNotify::new(timer.unpark()));

        // New Tokio reactor + threadpool
        let rt = tokio::runtime::Runtime::new()?;

        let timer_handle = timer.handle();

        // Executor to run !Send futures
        let executor = RefCell::new(CurrentThread::new_with_park(timer));

        // Used to send messages across threads
        let (tx, rx) = mpsc::unbounded();

        // Wrap the rx half with a future context and refcell
        let rx = RefCell::new(executor::spawn(rx));

        let id = NEXT_LOOP_ID.fetch_add(1, Ordering::Relaxed);

        Ok(Core {
            id,
            rt,
            notify_future,
            notify_rx,
            tx,
            rx,
            executor,
            timer_handle,
            inner: Rc::new(RefCell::new(Inner {
                pending_spawn: vec![],
            })),
        })
    }

    /// Returns a handle to this event loop which cannot be sent across threads
    /// but can be used as a proxy to the event loop itself.
    ///
    /// Handles are cloneable and clones always refer to the same event loop.
    /// This handle is typically passed into functions that create I/O objects
    /// to bind them to this event loop.
    pub fn handle(&self) -> Handle {
        Handle {
            remote: self.remote(),
            inner: Rc::downgrade(&self.inner),
            thread_pool: self.rt.executor().clone(),
        }
    }

    /// Returns a reference to the runtime backing the instance
    ///
    /// This provides access to the newer features of Tokio.
    pub fn runtime(&self) -> &tokio::runtime::Runtime {
        &self.rt
    }

    /// Generates a remote handle to this event loop which can be used to spawn
    /// tasks from other threads into this event loop.
    pub fn remote(&self) -> Remote {
        Remote {
            id: self.id,
            tx: self.tx.clone(),
            new_handle: self.rt.reactor().clone(),
            timer_handle: self.timer_handle.clone()
        }
    }

    /// Runs a future until completion, driving the event loop while we're
    /// otherwise waiting for the future to complete.
    ///
    /// This function will begin executing the event loop and will finish once
    /// the provided future is resolved. Note that the future argument here
    /// crucially does not require the `'static` nor `Send` bounds. As a result
    /// the future will be "pinned" to not only this thread but also this stack
    /// frame.
    ///
    /// This function will return the value that the future resolves to once
    /// the future has finished. If the future never resolves then this function
    /// will never return.
    ///
    /// # Panics
    ///
    /// This method will **not** catch panics from polling the future `f`. If
    /// the future panics then it's the responsibility of the caller to catch
    /// that panic and handle it as appropriate.
    pub fn run<F>(&mut self, f: F) -> Result<F::Item, F::Error>
        where F: Future,
    {
        let mut task = executor::spawn(f);
        let handle1 = self.rt.reactor().clone();
        let handle2 = self.rt.reactor().clone();
        let mut executor1 = self.rt.executor().clone();
        let mut executor2 = self.rt.executor().clone();
        let timer_handle = self.timer_handle.clone();

        // Make sure the future will run at least once on enter
        self.notify_future.notify(0);

        loop {
            if self.notify_future.take() {
                let mut enter = tokio_executor::enter()
                    .ok().expect("cannot recursively call into `Core`");

                let notify = &self.notify_future;
                let mut current_thread = self.executor.borrow_mut();

                let res = try!(CURRENT_LOOP.set(self, || {
                    ::tokio_reactor::with_default(&handle1, &mut enter, |enter| {
                        tokio_executor::with_default(&mut executor1, enter, |enter| {
                            timer::with_default(&timer_handle, enter, |enter| {
                                current_thread.enter(enter)
                                    .block_on(future::lazy(|| {
                                        Ok::<_, ()>(task.poll_future_notify(notify, 0))
                                    })).unwrap()
                            })
                        })
                    })
                }));

                if let Async::Ready(e) = res {
                    return Ok(e)
                }
            }

            self.poll(None, &handle2, &mut executor2);
        }
    }

    /// Performs one iteration of the event loop, blocking on waiting for events
    /// for at most `max_wait` (forever if `None`).
    ///
    /// It only makes sense to call this method if you've previously spawned
    /// a future onto this event loop.
    ///
    /// `loop { lp.turn(None) }` is equivalent to calling `run` with an
    /// empty future (one that never finishes).
    pub fn turn(&mut self, max_wait: Option<Duration>) {
        let handle = self.rt.reactor().clone();
        let mut executor = self.rt.executor().clone();
        self.poll(max_wait, &handle, &mut executor);
    }

    fn poll(&mut self, max_wait: Option<Duration>,
            handle: &tokio::reactor::Handle,
            sender: &mut tokio::runtime::TaskExecutor) {
        let mut enter = tokio_executor::enter()
            .ok().expect("cannot recursively call into `Core`");
        let timer_handle = self.timer_handle.clone();

        ::tokio_reactor::with_default(handle, &mut enter, |enter| {
            tokio_executor::with_default(sender, enter, |enter| {
                timer::with_default(&timer_handle, enter, |enter| {
                    let start = Instant::now();

                    // Process all the events that came in, dispatching appropriately
                    if self.notify_rx.take() {
                        CURRENT_LOOP.set(self, || self.consume_queue());
                    }

                    // Drain any futures pending spawn
                    {
                        let mut e = self.executor.borrow_mut();
                        let mut i = self.inner.borrow_mut();

                        for f in i.pending_spawn.drain(..) {
                            // Little hack
                            e.enter(enter).block_on(future::lazy(|| {
                                TaskExecutor::current().spawn_local(f).unwrap();
                                Ok::<_, ()>(())
                            })).unwrap();
                        }
                    }

                    CURRENT_LOOP.set(self, || {
                        self.executor.borrow_mut()
                            .enter(enter)
                            .turn(max_wait)
                            .ok().expect("error in `CurrentThread::turn`");
                    });

                    let after_poll = Instant::now();
                    debug!("loop poll - {:?}", after_poll - start);
                    debug!("loop time - {:?}", after_poll);

                    debug!("loop process, {:?}", after_poll.elapsed());
                })
            });
        });
    }

    fn consume_queue(&self) {
        debug!("consuming notification queue");
        // TODO: can we do better than `.unwrap()` here?
        loop {
            let msg = self.rx.borrow_mut().poll_stream_notify(&self.notify_rx, 0).unwrap();
            match msg {
                Async::Ready(Some(msg)) => self.notify(msg),
                Async::NotReady |
                Async::Ready(None) => break,
            }
        }
    }

    fn notify(&self, msg: Message) {
        let Message::Run(r) = msg;
        r.call_box(self);
    }

    /// Get the ID of this loop
    pub fn id(&self) -> CoreId {
        CoreId(self.id)
    }
}

impl<F> Executor<F> for Core
    where F: Future<Item = (), Error = ()> + 'static,
{
    fn execute(&self, future: F) -> Result<(), ExecuteError<F>> {
        self.handle().execute(future)
    }
}

impl fmt::Debug for Core {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Core")
         .field("id", &self.id())
         .finish()
    }
}

impl Remote {
    fn send(&self, msg: Message) {
        self.with_loop(|lp| {
            match lp {
                Some(lp) => {
                    // We want to make sure that all messages are received in
                    // order, so we need to consume pending messages before
                    // delivering this message to the core. The actually
                    // `consume_queue` function, however, can be somewhat slow
                    // right now where receiving on a channel will acquire a
                    // lock and block the current task.
                    //
                    // To speed this up check the message queue's readiness as a
                    // sort of preflight check to see if we've actually got any
                    // messages. This should just involve some atomics and if it
                    // comes back false then we know for sure there are no
                    // pending messages, so we can immediately deliver our
                    // message.
                    if lp.notify_rx.take() {
                        lp.consume_queue();
                    }
                    lp.notify(msg);
                }
                None => {
                    match self.tx.unbounded_send(msg) {
                        Ok(()) => {}

                        // TODO: this error should punt upwards and we should
                        //       notify the caller that the message wasn't
                        //       received. This is tokio-core#17
                        Err(e) => drop(e),
                    }
                }
            }
        })
    }

    fn with_loop<F, R>(&self, f: F) -> R
        where F: FnOnce(Option<&Core>) -> R
    {
        if CURRENT_LOOP.is_set() {
            CURRENT_LOOP.with(|lp| {
                let same = lp.id == self.id;
                if same {
                    f(Some(lp))
                } else {
                    f(None)
                }
            })
        } else {
            f(None)
        }
    }

    /// Spawns a new future into the event loop this remote is associated with.
    ///
    /// This function takes a closure which is executed within the context of
    /// the I/O loop itself. The future returned by the closure will be
    /// scheduled on the event loop and run to completion.
    ///
    /// Note that while the closure, `F`, requires the `Send` bound as it might
    /// cross threads, the future `R` does not.
    ///
    /// # Panics
    ///
    /// This method will **not** catch panics from polling the future `f`. If
    /// the future panics then it's the responsibility of the caller to catch
    /// that panic and handle it as appropriate.
    pub fn spawn<F, R>(&self, f: F)
        where F: FnOnce(&Handle) -> R + Send + 'static,
              R: IntoFuture<Item=(), Error=()>,
              R::Future: 'static,
    {
        self.send(Message::Run(Box::new(|lp: &Core| {
            let f = f(&lp.handle());
            lp.handle().spawn(f.into_future());
        })));
    }

    /// Return the ID of the represented Core
    pub fn id(&self) -> CoreId {
        CoreId(self.id)
    }

    /// Attempts to "promote" this remote to a handle, if possible.
    ///
    /// This function is intended for structures which typically work through a
    /// `Remote` but want to optimize runtime when the remote doesn't actually
    /// leave the thread of the original reactor. This will attempt to return a
    /// handle if the `Remote` is on the same thread as the event loop and the
    /// event loop is running.
    ///
    /// If this `Remote` has moved to a different thread or if the event loop is
    /// running, then `None` may be returned. If you need to guarantee access to
    /// a `Handle`, then you can call this function and fall back to using
    /// `spawn` above if it returns `None`.
    pub fn handle(&self) -> Option<Handle> {
        if CURRENT_LOOP.is_set() {
            CURRENT_LOOP.with(|lp| {
                let same = lp.id == self.id;
                if same {
                    Some(lp.handle())
                } else {
                    None
                }
            })
        } else {
            None
        }
    }
}

impl<F> Executor<F> for Remote
    where F: Future<Item = (), Error = ()> + Send + 'static,
{
    fn execute(&self, future: F) -> Result<(), ExecuteError<F>> {
        self.spawn(|_| future);
        Ok(())
    }
}

impl fmt::Debug for Remote {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Remote")
         .field("id", &self.id())
         .finish()
    }
}

impl Handle {
    /// Returns a reference to the new Tokio handle
    pub fn new_tokio_handle(&self) -> &::tokio::reactor::Handle {
        &self.remote.new_handle
    }

    /// Returns a reference to the underlying remote handle to the event loop.
    pub fn remote(&self) -> &Remote {
        &self.remote
    }

    /// Spawns a new future on the event loop this handle is associated with.
    ///
    /// # Panics
    ///
    /// This method will **not** catch panics from polling the future `f`. If
    /// the future panics then it's the responsibility of the caller to catch
    /// that panic and handle it as appropriate.
    pub fn spawn<F>(&self, f: F)
        where F: Future<Item=(), Error=()> + 'static,
    {
        let inner = match self.inner.upgrade() {
            Some(inner) => inner,
            None => {
                return;
            }
        };

        // Try accessing the executor directly
        if let Ok(mut inner) = inner.try_borrow_mut() {
            inner.pending_spawn.push(Box::new(f));
            return;
        }

        // If that doesn't work, the executor is probably active, so spawn using
        // the global fn.
        let _ = TaskExecutor::current().spawn_local(Box::new(f));
    }

    /// Spawns a new future onto the threadpool
    ///
    /// # Panics
    ///
    /// This function panics if the spawn fails. Failure occurs if the executor
    /// is currently at capacity and is unable to spawn a new future.
    pub fn spawn_send<F>(&self, f: F)
        where F: Future<Item=(), Error=()> + Send + 'static,
    {
        self.thread_pool.spawn(f);
    }

    /// Spawns a closure on this event loop.
    ///
    /// This function is a convenience wrapper around the `spawn` function above
    /// for running a closure wrapped in `futures::lazy`. It will spawn the
    /// function `f` provided onto the event loop, and continue to run the
    /// future returned by `f` on the event loop as well.
    ///
    /// # Panics
    ///
    /// This method will **not** catch panics from polling the future `f`. If
    /// the future panics then it's the responsibility of the caller to catch
    /// that panic and handle it as appropriate.
    pub fn spawn_fn<F, R>(&self, f: F)
        where F: FnOnce() -> R + 'static,
              R: IntoFuture<Item=(), Error=()> + 'static,
    {
        self.spawn(future::lazy(f))
    }

    /// Return the ID of the represented Core
    pub fn id(&self) -> CoreId {
        self.remote.id()
    }
}

impl<F> Executor<F> for Handle
    where F: Future<Item = (), Error = ()> + 'static,
{
    fn execute(&self, future: F) -> Result<(), ExecuteError<F>> {
        self.spawn(future);
        Ok(())
    }
}

impl fmt::Debug for Handle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Handle")
         .field("id", &self.id())
         .finish()
    }
}

struct MyNotify {
    unpark: UnparkThread,
    notified: AtomicBool,
}

impl MyNotify {
    fn new(unpark: UnparkThread) -> Self {
        MyNotify {
            unpark,
            notified: AtomicBool::new(true),
        }
    }

    fn take(&self) -> bool {
        self.notified.swap(false, Ordering::SeqCst)
    }
}

impl Notify for MyNotify {
    fn notify(&self, _: usize) {
        self.notified.store(true, Ordering::SeqCst);
        self.unpark.unpark();
    }
}

trait FnBox: Send + 'static {
    fn call_box(self: Box<Self>, lp: &Core);
}

impl<F: FnOnce(&Core) + Send + 'static> FnBox for F {
    fn call_box(self: Box<Self>, lp: &Core) {
        (*self)(lp)
    }
}

const READ: usize = 1 << 0;
const WRITE: usize = 1 << 1;

fn ready2usize(ready: mio::Ready) -> usize {
    let mut bits = 0;
    if ready.is_readable() {
        bits |= READ;
    }
    if ready.is_writable() {
        bits |= WRITE;
    }
    bits | platform::ready2usize(ready)
}

fn usize2ready(bits: usize) -> mio::Ready {
    let mut ready = mio::Ready::empty();
    if bits & READ != 0 {
        ready.insert(mio::Ready::readable());
    }
    if bits & WRITE != 0 {
        ready.insert(mio::Ready::writable());
    }
    ready | platform::usize2ready(bits)
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
mod platform {
    use mio::Ready;
    use mio::unix::UnixReady;

    const HUP: usize = 1 << 2;
    const ERROR: usize = 1 << 3;
    const AIO: usize = 1 << 4;

    #[cfg(any(target_os = "dragonfly", target_os = "freebsd"))]
    fn is_aio(ready: &Ready) -> bool {
        UnixReady::from(*ready).is_aio()
    }

    #[cfg(not(any(target_os = "dragonfly", target_os = "freebsd")))]
    fn is_aio(_ready: &Ready) -> bool {
        false
    }

    pub fn ready2usize(ready: Ready) -> usize {
        let ready = UnixReady::from(ready);
        let mut bits = 0;
        if is_aio(&ready) {
            bits |= AIO;
        }
        if ready.is_error() {
            bits |= ERROR;
        }
        if ready.is_hup() {
            bits |= HUP;
        }
        bits
    }

    #[cfg(any(target_os = "dragonfly", target_os = "freebsd", target_os = "ios",
              target_os = "macos"))]
    fn usize2ready_aio(ready: &mut UnixReady) {
        ready.insert(UnixReady::aio());
    }

    #[cfg(not(any(target_os = "dragonfly",
        target_os = "freebsd", target_os = "ios", target_os = "macos")))]
    fn usize2ready_aio(_ready: &mut UnixReady) {
        // aio not available here → empty
    }

    pub fn usize2ready(bits: usize) -> Ready {
        let mut ready = UnixReady::from(Ready::empty());
        if bits & AIO != 0 {
            usize2ready_aio(&mut ready);
        }
        if bits & HUP != 0 {
            ready.insert(UnixReady::hup());
        }
        if bits & ERROR != 0 {
            ready.insert(UnixReady::error());
        }
        ready.into()
    }
}

#[cfg(any(windows, target_os = "fuchsia"))]
mod platform {
    use mio::Ready;

    pub fn ready2usize(_r: Ready) -> usize {
        0
    }

    pub fn usize2ready(_r: usize) -> Ready {
        Ready::empty()
    }
}
