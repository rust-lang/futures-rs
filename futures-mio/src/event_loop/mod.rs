use std::cell::RefCell;
use std::io::{self, ErrorKind};
use std::marker;
use std::mem;
use std::rc::Rc;
use std::sync::{Mutex, Arc};
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};
use std::time::{Instant, Duration};

use futures::{Future, Poll};
use futures::task::{self, Task, Unpark};
use mio;
use slab::Slab;

use slot::{self, Slot};
use timer_wheel::{TimerWheel, Timeout};

mod channel;
mod loop_task;
mod source;
mod timeout;
pub use self::source::{AddSource, IoToken};
pub use self::timeout::{AddTimeout, TimeoutToken};
use self::channel::{Sender, Receiver, channel};
use self::loop_task::LoopTask;

static NEXT_LOOP_ID: AtomicUsize = ATOMIC_USIZE_INIT;
scoped_thread_local!(static CURRENT_LOOP_DATA: LoopData);

const SLAB_CAPACITY: usize = 1024 * 64;

/// An event loop.
///
/// The event loop is the main source of blocking in an application which drives
/// all other I/O events and notifications happening. Each event loop can have
/// multiple handles pointing to it, each of which can then be used to create
/// various I/O objects to interact with the event loop in interesting ways.
// TODO: expand this
pub struct Loop<'a> {
    data: Arc<LoopData>,

    // A `Loop` cannot be sent to other threads as it's used as a proxy for data
    // that belongs to the thread the loop was running on at some point.
    // Also, be invariant over 'a.
    _marker: marker::PhantomData<(Rc<u32>, *mut &'a ())>,
}

/// The inner data of an event loop, which is also accessible from loop handles.
struct LoopData {
    id: usize,
    io: mio::Poll,
    events: RefCell<mio::Events>,
    dispatch: RefCell<Slab<Scheduled, usize>>,
    _future_registration: mio::Registration,
    future_readiness: Arc<mio::SetReadiness>,
    queued_tasks: RefCell<Vec<Arc<LoopTask>>>,

    tx: Arc<Sender<Message>>,
    rx: Receiver<Message>,

    // Timer wheel keeping track of all timeouts. The `usize` stored in the
    // timer wheel is an index into the slab below.
    //
    // The slab below keeps track of the timeouts themselves as well as the
    // state of the timeout itself. The `TimeoutToken` type is an index into the
    // `timeouts` slab.
    timer_wheel: RefCell<TimerWheel<usize>>,
    timeouts: RefCell<Slab<(Timeout, TimeoutState), usize>>,
}

/// Handle to an event loop, used to construct I/O objects, send messages, and
/// otherwise interact indirectly with the event loop itself.
///
/// Handles can be cloned, and when cloned they will still refer to the
/// same underlying event loop.
#[derive(Clone)]
pub struct LoopHandle<'a> {
    id: usize,
    tx: Arc<Sender<Message>>,
    // Invariant over 'a
    _marker: marker::PhantomData<Mutex<&'a ()>>,
}

/// A non-sendable, cloneable handle to an event loop, useful for executing
/// non-send Futures that live on the event loop.
#[derive(Clone)]
pub struct LoopPin<'a> {
    data: Arc<LoopData>,

    // A `LoopPin` cannot be sent to other threads as it's used as a proxy for
    // data that belongs to the thread the loop was running on at some point.
    // Also, be invariant over 'a.
    _marker: marker::PhantomData<(Rc<u32>, Mutex<&'a ()>)>,
}

struct Scheduled {
    readiness: Arc<AtomicUsize>,
    reader: Option<Arc<Unpark>>,
    writer: Option<Arc<Unpark>>,
}

enum TimeoutState {
    NotFired,
    Fired,
    Waiting(Arc<Unpark>),
}

enum Direction {
    Read,
    Write,
}

enum Message {
    DropSource(usize),
    Schedule(usize, Arc<Unpark>, Direction),
    AddTimeout(Instant, Arc<Slot<io::Result<(usize, Instant)>>>),
    UpdateTimeout(usize, Arc<Unpark>),
    CancelTimeout(usize),
    Run(Box<ExecuteCallback>),
    RunTask(Arc<LoopTask>),
}

/// Essentially `Box<FnOnce() + Send>`, just as a trait.
trait ExecuteCallback: Send {
    #[allow(missing_docs)]
    fn call(self: Box<Self>);
}

impl<F: FnOnce() + Send> ExecuteCallback for F {
    fn call(self: Box<F>) {
        (*self)()
    }
}

impl<'a> Loop<'a> {
    /// Creates a new event loop, returning any error that happened during the
    /// creation.
    pub fn new() -> io::Result<Loop<'a>> {
        let (tx, rx) = channel();
        let io = try!(mio::Poll::new());
        try!(io.register(&rx,
                         mio::Token(0),
                         mio::EventSet::readable(),
                         mio::PollOpt::edge()));
        let pair = mio::Registration::new(&io,
                                          mio::Token(1),
                                          mio::EventSet::readable(),
                                          mio::PollOpt::level());
        let (registration, readiness) = pair;
        let data = LoopData {
            id: NEXT_LOOP_ID.fetch_add(1, Ordering::Relaxed),
            io: io,
            events: RefCell::new(mio::Events::new()),
            queued_tasks: RefCell::new(Vec::new()),
            tx: Arc::new(tx),
            rx: rx,
            _future_registration: registration,
            future_readiness: Arc::new(readiness),
            dispatch: RefCell::new(Slab::new_starting_at(2, SLAB_CAPACITY)),
            timeouts: RefCell::new(Slab::new_starting_at(0, SLAB_CAPACITY)),
            timer_wheel: RefCell::new(TimerWheel::new()),
        };
        Ok(Loop {
            data: Arc::new(data),
            _marker: marker::PhantomData,
        })
    }

    /// Generates a handle to this event loop used to construct I/O objects and
    /// send messages.
    ///
    /// Handles to an event loop are cloneable as well and clones will always
    /// refer to the same event loop.
    pub fn handle(&self) -> LoopHandle<'a> {
        LoopHandle {
            id: self.data.id,
            tx: self.data.tx.clone(),
            _marker: marker::PhantomData,
        }
    }

    /// Returns a "pin" of this event loop which cannot be sent across threads
    /// but can be used as a proxy to the event loop itself.
    ///
    /// Currently the primary use for this is as an executor that handles
    /// non-`Send`, non-`'static` futures that are pinned to the event loop
    /// thread.
    pub fn pin(&self) -> LoopPin<'a> {
        LoopPin {
            data: self.data.clone(),
            _marker: marker::PhantomData,
        }
    }

    /// Runs a future until completion, driving the event loop while we're
    /// otherwise waiting for the future to complete.
    ///
    /// This function will begin executing the event loop and will finish once
    /// the provided future is resolve. Note that the future argument here
    /// crucially does not require the `'static` nor `Send` bounds. As a result
    /// the future will be "pinned" to not only this thread but also this stack
    /// frame.
    ///
    /// This function will returns the value that the future resolves to once
    /// the future has finished. If the future never resolves then this function
    /// will never return.
    ///
    /// # Panics
    ///
    /// This method will **not** catch panics from polling the future `f`. If
    /// the future panics then it's the responsibility of the caller to catch
    /// that panic and handle it as appropriate.
    ///
    /// Similarly, becuase the provided future will be pinned not only to this
    /// thread but also to this task, any attempt to poll the future on a
    /// separate thread will result in a panic. That is, calls to
    /// `task::poll_on` must be avoided.
    pub fn run<F>(&mut self, mut f: F) -> Result<F::Item, F::Error>
        where F: Future,
    {
        struct MyNotify(Arc<mio::SetReadiness>);

        impl Unpark for MyNotify {
            fn unpark(&self) {
                self.0.set_readiness(mio::EventSet::readable())
                      .expect("failed to set readiness");
            }
        }

        // First up, create the task that will drive this future. The task here
        // isn't a "normal task" but rather one where we define what to do when
        // a readiness notification comes in.
        //
        // We translate readiness notifications to a `set_readiness` of our
        // `future_readiness` structure we have stored internally.
        let mut task = Task::new();
        let notify: Arc<Unpark> = Arc::new(MyNotify(self.data.future_readiness.clone()));
        let ready = self.data.future_readiness.clone();

        // Next, move all that data into a dynamically dispatched closure to cut
        // down on monomorphization costs. Inside this closure we unset the
        // readiness of the future (as we're about to poll it) and then we check
        // to see if it's done. If it's not then the event loop will turn again.
        let mut res = None;
        self._run(&mut || {
            ready.set_readiness(mio::EventSet::none())
                 .expect("failed to set readiness");
            assert!(res.is_none());
            match task.enter(&notify, || f.poll()) {
                Poll::NotReady => {}
                Poll::Ok(e) => res = Some(Ok(e)),
                Poll::Err(e) => res = Some(Err(e)),
            }
            res.is_some()
        });
        res.expect("run should not return until future is done")
    }

    fn _run(&mut self, done: &mut FnMut() -> bool) {
        // Check to see if we're done immediately, if so we shouldn't do any
        // work.
        if CURRENT_LOOP_DATA.set(&self.data, || done()) {
            return
        }

        loop {
            let amt;
            // On Linux, Poll::poll is epoll_wait, which may return EINTR if a
            // ptracer attaches. This retry loop prevents crashing when
            // attaching strace, or similar.
            let start = Instant::now();
            loop {
                let timeout = self.data.timer_wheel.borrow().next_timeout().map(|t| {
                    if t < start {
                        Duration::new(0, 0)
                    } else {
                        t - start
                    }
                });
                match self.data.io.poll(&mut self.data.events.borrow_mut(), timeout) {
                    Ok(a) => {
                        amt = a;
                        break;
                    }
                    Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                    err @ Err(_) => {
                        err.unwrap();
                    }
                }
            }
            debug!("loop poll - {:?}", start.elapsed());
            debug!("loop time - {:?}", Instant::now());

            // First up, process all timeouts that may have just occurred.
            let start = Instant::now();
            self.consume_timeouts(start);

            // Next, process all the events that came in.
            for i in 0..self.data.events.borrow().len() {
                let event = self.data.events.borrow().get(i).unwrap();
                let token = usize::from(event.token());

                // Token 0 == our incoming message queue, so this means we
                // process the whole queue of messages.
                //
                // Token 1 == we should poll the future, we'll do that right
                // after we get through the rest of this tick of the event loop.
                if token == 0 {
                    debug!("consuming notification queue");
                    CURRENT_LOOP_DATA.set(&self.data, || {
                        self.data.consume_queue();
                    });
                    continue
                } else if token == 1 {
                    if CURRENT_LOOP_DATA.set(&self.data, || done()) {
                        return
                    }
                    continue
                }

                trace!("event {:?} {:?}", event.kind(), event.token());

                // For any other token we look at `dispatch` to see what we're
                // supposed to do. If there's a waiter we get ready to notify
                // it, and we also or-in atomically any events that have
                // happened (currently read/write events).
                let mut reader = None;
                let mut writer = None;
                if let Some(sched) = self.data.dispatch.borrow_mut().get_mut(token) {
                    if event.kind().is_readable() {
                        reader = sched.reader.take();
                        sched.readiness.fetch_or(1, Ordering::Relaxed);
                    }
                    if event.kind().is_writable() {
                        writer = sched.writer.take();
                        sched.readiness.fetch_or(2, Ordering::Relaxed);
                    }
                } else {
                    debug!("notified on {} which no longer exists", token);
                }

                // If we actually got a waiter, then notify!
                //
                // TODO: don't notify the same task twice
                if let Some(reader) = reader {
                    self.data.notify_handle(reader);
                }
                if let Some(writer) = writer {
                    self.data.notify_handle(writer);
                }
            }

            // Finally, process any queued tasks
            while let Some(task) = self.data.queued_tasks.borrow_mut().pop() {
                CURRENT_LOOP_DATA.set(&self.data, || unsafe {
                    // safe because we *are* the event loop
                    loop_task::run(task)
                });
            }

            debug!("loop process - {} events, {:?}", amt, start.elapsed());
        }
    }

    fn consume_timeouts(&mut self, now: Instant) {
        loop {
            let idx = match self.data.timer_wheel.borrow_mut().poll(now) {
                Some(idx) => idx,
                None => break,
            };
            trace!("firing timeout: {}", idx);
            let handle = self.data.timeouts.borrow_mut()[idx].1.fire();
            if let Some(handle) = handle {
                self.data.notify_handle(handle);
            }
        }
    }
}

impl LoopData {
    /// Method used to notify a task handle.
    ///
    /// Note that this should be used instead fo `handle.unpark()` to ensure
    /// that the `CURRENT_LOOP_ID` variable is set appropriately.
    pub fn notify_handle(&self, handle: Arc<Unpark>) {
        debug!("notifying a task handle");
        CURRENT_LOOP_DATA.set(self, || handle.unpark());
    }

    fn add_source(&self, source: &mio::Evented)
                  -> io::Result<(Arc<AtomicUsize>, usize)> {
        debug!("adding a new I/O source");
        let sched = Scheduled {
            readiness: Arc::new(AtomicUsize::new(0)),
            reader: None,
            writer: None,
        };
        let mut dispatch = self.dispatch.borrow_mut();
        if dispatch.vacant_entry().is_none() {
            let amt = dispatch.count();
            dispatch.grow(amt);
        }
        let entry = dispatch.vacant_entry().unwrap();
        try!(self.io.register(source,
                              mio::Token(entry.index()),
                              mio::EventSet::readable() |
                              mio::EventSet::writable(),
                              mio::PollOpt::edge()));
        Ok((sched.readiness.clone(), entry.insert(sched).index()))
    }

    fn drop_source(&self, token: usize) {
        debug!("dropping I/O source: {}", token);
        self.dispatch.borrow_mut().remove(token).unwrap();
    }

    fn schedule(&self, token: usize, wake: Arc<Unpark>, dir: Direction) {
        debug!("scheduling direction for: {}", token);
        let to_call = {
            let mut dispatch = self.dispatch.borrow_mut();
            let sched = dispatch.get_mut(token).unwrap();
            let (slot, bit) = match dir {
                Direction::Read => (&mut sched.reader, 1),
                Direction::Write => (&mut sched.writer, 2),
            };
            let ready = sched.readiness.load(Ordering::SeqCst);
            if ready & bit != 0 {
                *slot = None;
                sched.readiness.store(ready & !bit, Ordering::SeqCst);
                Some(wake)
            } else {
                *slot = Some(wake);
                None
            }
        };
        if let Some(to_call) = to_call {
            debug!("schedule immediately done");
            self.notify_handle(to_call);
        }
    }

    fn add_timeout(&self, at: Instant) -> io::Result<(usize, Instant)> {
        let mut timeouts = self.timeouts.borrow_mut();
        if timeouts.vacant_entry().is_none() {
            let len = timeouts.count();
            timeouts.grow(len);
        }
        let entry = timeouts.vacant_entry().unwrap();
        let timeout = self.timer_wheel.borrow_mut().insert(at, entry.index());
        let when = *timeout.when();
        let entry = entry.insert((timeout, TimeoutState::NotFired));
        debug!("added a timeout: {}", entry.index());
        Ok((entry.index(), when))
    }

    fn update_timeout(&self, token: usize, handle: Arc<Unpark>) {
        debug!("updating a timeout: {}", token);
        let to_wake = self.timeouts.borrow_mut()[token].1.block(handle);
        if let Some(to_wake) = to_wake {
            self.notify_handle(to_wake);
        }
    }

    fn cancel_timeout(&self, token: usize) {
        debug!("cancel a timeout: {}", token);
        let pair = self.timeouts.borrow_mut().remove(token);
        if let Some((timeout, _state)) = pair {
            self.timer_wheel.borrow_mut().cancel(&timeout);
        }
    }

    fn notify(&self, msg: Message) {
        match msg {
            Message::DropSource(tok) => self.drop_source(tok),
            Message::Schedule(tok, wake, dir) => self.schedule(tok, wake, dir),
            Message::AddTimeout(at, slot) => {
                slot.try_produce(self.add_timeout(at))
                    .ok().expect("interference with try_produce on timeout");
            }
            Message::UpdateTimeout(t, handle) => self.update_timeout(t, handle),
            Message::CancelTimeout(t) => self.cancel_timeout(t),
            Message::Run(f) => {
                debug!("running a closure");
                f.call()
            }
            Message::RunTask(t) => {
                self.queued_tasks.borrow_mut().push(t)
            }
        }
    }

    fn consume_queue(&self) {
        // TODO: can we do better than `.unwrap()` here?
        while let Some(msg) = self.rx.recv().unwrap() {
            self.notify(msg);
        }
    }
}


impl<'a> LoopHandle<'a> {
    fn send(&self, msg: Message) {
        self.with_loop(|lp| {
            match lp {
                Some(lp) => {
                    // Need to execute all existing requests first, to ensure
                    // that our message is processed "in order"
                    lp.consume_queue();
                    lp.notify(msg);
                }
                None => {
                    match self.tx.send(msg) {
                        Ok(()) => {}

                        // This should only happen when there was an error
                        // writing to the pipe to wake up the event loop,
                        // hopefully that never happens
                        Err(e) => {
                            panic!("error sending message to event loop: {}", e)
                        }
                    }
                }
            }
        })
    }


    fn with_loop<F, R>(&self, f: F) -> R
        where F: FnOnce(Option<&LoopData>) -> R
    {
        if CURRENT_LOOP_DATA.is_set() {
            CURRENT_LOOP_DATA.with(|lp| {
                if lp.id == self.id {
                    f(Some(lp))
                } else {
                    f(None)
                }
            })
        } else {
            f(None)
        }
    }

    /// Erases the ability to execute tasks with a non-`'static`
    /// lifetime. Occasionally useful when you need to uniformly work with a
    /// `'static` version of the type.
    pub fn into_static(self) -> LoopHandle<'static> {
        // This is justified by the fact that the LoopHandle *is* actually
        // 'static internally; the 'a is a phantom parameter that is intended to
        // limit what futures can be executed by it.
        unsafe { ::std::mem::transmute(self) }
    }

}

impl<'a> LoopPin<'a> {
    /// Returns a reference to the underlying handle to the event loop.
    pub fn handle(&self) -> LoopHandle<'a> {
        LoopHandle {
            id: self.data.id,
            tx: self.data.tx.clone(),
            _marker: marker::PhantomData,
        }
    }
}

struct LoopFuture<'a, T, U> {
    loop_handle: LoopHandle<'a>,
    data: Option<U>,
    result: Option<(Arc<Slot<io::Result<T>>>, slot::Token)>,
}

impl<'a, T, U> LoopFuture<'a, T, U>
    where T: 'a,
{
    fn poll<F, G>(&mut self, f: F, g: G) -> Poll<T, io::Error>
        where F: FnOnce(&LoopData, U) -> io::Result<T>,
              G: FnOnce(U, Arc<Slot<io::Result<T>>>) -> Message,
    {
        match self.result {
            Some((ref result, ref mut token)) => {
                result.cancel(*token);
                match result.try_consume() {
                    Ok(t) => return t.into(),
                    Err(_) => {}
                }
                let task = task::park();
                *token = result.on_full(move |_| {
                    task.unpark();
                });
                return Poll::NotReady
            }
            None => {
                let data = &mut self.data;
                let ret = self.loop_handle.with_loop(|lp| {
                    lp.map(|lp| f(lp, data.take().unwrap()))
                });
                if let Some(ret) = ret {
                    debug!("loop future done immediately on event loop");
                    return ret.into()
                }
                debug!("loop future needs to send info to event loop");

                let task = task::park();
                let result = Arc::new(Slot::new(None));
                let token = result.on_full(move |_| {
                    task.unpark();
                });
                self.result = Some((result.clone(), token));
                self.loop_handle.send(g(data.take().unwrap(), result));
                Poll::NotReady
            }
        }
    }
}

impl TimeoutState {
    fn block(&mut self, handle: Arc<Unpark>) -> Option<Arc<Unpark>> {
        match *self {
            TimeoutState::Fired => return Some(handle),
            _ => {}
        }
        *self = TimeoutState::Waiting(handle);
        None
    }

    fn fire(&mut self) -> Option<Arc<Unpark>> {
        match mem::replace(self, TimeoutState::Fired) {
            TimeoutState::NotFired => None,
            TimeoutState::Fired => panic!("fired twice?"),
            TimeoutState::Waiting(handle) => Some(handle),
        }
    }
}
