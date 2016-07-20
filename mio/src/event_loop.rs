use std::cell::{Cell, RefCell};
use std::io::{self, ErrorKind};
use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};
use std::sync::mpsc;
use std::time::Instant;

use mio;
use mio::channel::SendError;
use slab::Slab;
use futures::{Future, Tokens, Wake, Poll};

use slot::{self, Slot};

pub type Source = Arc<mio::Evented + Send + Sync>;

static NEXT_LOOP_ID: AtomicUsize = ATOMIC_USIZE_INIT;
scoped_thread_local!(static CURRENT_LOOP: Loop);

const SLAB_CAPACITY: usize = 1024 * 64;

/// An event loop.
///
/// The event loop is the main source of blocking in an application which drives
/// all other I/O events and notifications happening. Each event loop can have
/// multiple handles pointing to it, each of which can then be used to create
/// various I/O objects to interact with the event loop in interesting ways.
// TODO: expand this
pub struct Loop {
    id: usize,
    active: Cell<bool>,
    io: RefCell<mio::Poll>,
    tx: mio::channel::Sender<Message>,
    rx: mio::channel::Receiver<Message>,
    dispatch: RefCell<Slab<Scheduled, usize>>,
}

/// Handle to an event loop, used to construct I/O objects, send messages, and
/// otherwise interact indirectly with the event loop itself.
///
/// Handles can be cloned, and when cloned they will still refer to the
/// same underlying event loop.
#[derive(Clone)]
pub struct LoopHandle {
    id: usize,
    tx: mio::channel::Sender<Message>,
}

#[allow(missing_docs)]
#[derive(Copy, Clone, Debug)]
pub enum Direction {
    Read,
    Write,
}

struct Scheduled {
    source: Source,
    reader: Half,
    writer: Half,
}

enum Half {
    NotReady,
    Ready,
    Waiting(Arc<Wake>),
}

impl Scheduled {
    fn waiter_for(&mut self, dir: Direction) -> &mut Half {
        match dir {
            Direction::Read => &mut self.reader,
            Direction::Write => &mut self.writer,
        }
    }
}

enum Message {
    AddSource(Source, Arc<Slot<io::Result<usize>>>),
    DropSource(usize),
    Schedule(usize, Direction, Arc<Wake>),
    Deschedule(usize, Direction),
    Shutdown,
}

fn register(poll: &mut mio::Poll,
            token: usize,
            sched: &Scheduled) -> io::Result<()> {
    poll.register(&*sched.source,
                  mio::Token(token),
                  mio::EventSet::readable() | mio::EventSet::writable(),
                  mio::PollOpt::edge())
}

fn deregister(poll: &mut mio::Poll, sched: &Scheduled) {
    // TODO: handle error
    poll.deregister(&*sched.source).unwrap();
}

impl Loop {
    /// Creates a new event loop, returning any error that happened during the
    /// creation.
    pub fn new() -> io::Result<Loop> {
        let (tx, rx) = mio::channel::from_std_channel(mpsc::channel());
        let io = try!(mio::Poll::new());
        try!(io.register(&rx,
                         mio::Token(0),
                         mio::EventSet::readable(),
                         mio::PollOpt::edge()));
        Ok(Loop {
            id: NEXT_LOOP_ID.fetch_add(1, Ordering::Relaxed),
            active: Cell::new(true),
            io: RefCell::new(io),
            tx: tx,
            rx: rx,
            dispatch: RefCell::new(Slab::new_starting_at(1, SLAB_CAPACITY)),
        })
    }

    /// Generates a handle to this event loop used to construct I/O objects and
    /// send messages.
    ///
    /// Handles to an event loop are cloneable as well and clones will always
    /// refer to the same event loop.
    pub fn handle(&self) -> LoopHandle {
        LoopHandle {
            id: self.id,
            tx: self.tx.clone(),
        }
    }

    #[allow(missing_docs)]
    pub fn run<F: Future>(&mut self, f: F) -> Result<F::Item, F::Error> {
        let (tx_res, rx_res) = mpsc::channel();
        let handle = self.handle();
        f.then(move |res| {
            handle.shutdown();
            tx_res.send(res)
        }).forget();

        while self.active.get() {
            let amt;
            // On Linux, Poll::poll is epoll_wait, which may return EINTR if a
            // ptracer attaches. This retry loop prevents crashing when
            // attaching strace, or similar.
            let start = Instant::now();
            loop {
                match self.io.borrow_mut().poll(None) {
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

            // TODO: coalesce token sets for a given Wake?
            let start = Instant::now();
            for i in 0..amt {
                let event = self.io.borrow_mut().events().get(i).unwrap();
                let token = event.token().as_usize();

                if token == 0 {
                    self.consume_queue();
                } else {
                    let mut reader = None;
                    let mut writer = None;

                    let mut tokens = Tokens::empty();
                    if let Some(sched) = self.dispatch.borrow_mut().get_mut(token) {
                        if event.kind().is_readable() {
                            reader = sched.reader.set();
                            tokens.insert(2 * token);
                        }

                        if event.kind().is_writable() {
                            writer = sched.writer.set();
                            tokens.insert(2 * token + 1);
                        }
                    } else {
                        debug!("notified on {} which no longer exists", token);
                    }

                    CURRENT_LOOP.set(&self, || {
                        match (reader, writer) {
                            (Some(r), Some(w)) => {
                                r.wake(&tokens);
                                if &*r as *const Wake != &*w as *const Wake {
                                    w.wake(&tokens);
                                }
                            }
                            (Some(r), None) => r.wake(&tokens),
                            (None, Some(w)) => w.wake(&tokens),
                            (None, None) => {}
                        }
                    });
                }
            }

            debug!("loop process - {} events, {:?}", amt, start.elapsed());
        }

        rx_res.recv().unwrap()
    }

    fn add_source(&self, source: Source) -> io::Result<usize> {
        let sched = Scheduled {
            source: source,
            reader: Half::NotReady,
            writer: Half::NotReady,
        };
        let mut dispatch = self.dispatch.borrow_mut();
        if dispatch.vacant_entry().is_none() {
            let amt = dispatch.count();
            dispatch.grow(amt);
        }
        let entry = dispatch.vacant_entry().unwrap();
        try!(register(&mut self.io.borrow_mut(), entry.index(), &sched));
        Ok(entry.insert(sched).index())
    }

    fn drop_source(&self, token: usize) {
        let sched = self.dispatch.borrow_mut().remove(token).unwrap();
        deregister(&mut self.io.borrow_mut(), &sched);
    }

    fn schedule(&self, token: usize, dir: Direction, wake: Arc<Wake>) {
        let to_call = {
            let mut dispatch = self.dispatch.borrow_mut();
            let sched = dispatch.get_mut(token).unwrap();
            sched.waiter_for(dir).block(Some(wake))
        };
        if let Some(to_call) = to_call {
            debug!("immediate wakeup on {:?}", dir);

            let mut tokens = Tokens::empty();
            let token = match dir {
                Direction::Read => 2 * token,
                Direction::Write => 2 * token + 1,
            };
            tokens.insert(token);
            to_call.wake(&tokens);
        }
    }

    fn deschedule(&self, token: usize, dir: Direction) {
        let mut dispatch = self.dispatch.borrow_mut();
        let sched = dispatch.get_mut(token).unwrap();
        assert!(sched.waiter_for(dir).block(None).is_none());
    }

    fn consume_queue(&self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.notify(msg);
        }
    }

    fn notify(&self, msg: Message) {
        match msg {
            Message::AddSource(source, slot) => {
                // This unwrap() should always be ok as we're the only producer
                slot.try_produce(self.add_source(source))
                    .ok().expect("interference with try_produce");
            }
            Message::DropSource(tok) => self.drop_source(tok),
            Message::Schedule(tok, dir, wake) => self.schedule(tok, dir, wake),
            Message::Deschedule(tok, dir) => self.deschedule(tok, dir),
            Message::Shutdown => self.active.set(false),
        }
    }
}

impl Half {
    fn set(&mut self) -> Option<Arc<Wake>> {
        match mem::replace(self, Half::Ready) {
            Half::NotReady => None,
            Half::Ready => None,
            Half::Waiting(arc) => {
                *self = Half::NotReady;
                Some(arc)
            }
        }
    }

    fn block(&mut self, waiter: Option<Arc<Wake>>) -> Option<Arc<Wake>> {
        match (&*self, waiter) {
            (&Half::NotReady, None) => {}
            (&Half::NotReady, Some(other)) => {
                *self = Half::Waiting(other)
            }
            (&Half::Ready, None) => {}
            (&Half::Ready, Some(other)) => return Some(other),
            (&Half::Waiting(..), None) => *self = Half::NotReady,
            (&Half::Waiting(..), Some(other)) => *self = Half::Waiting(other),
        }
        None
    }
}

impl LoopHandle {
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
                        Err(SendError::Io(e)) => {
                            panic!("error sending message to event loop: {}", e)
                        }

                        // If we're still sending a message to the event loop
                        // after it's closed, then that's bad!
                        Err(SendError::Disconnected(_)) => {
                            panic!("event loop is no longer available")
                        }
                    }
                }
            }
        })
    }

    fn with_loop<F, R>(&self, f: F) -> R
        where F: FnOnce(Option<&Loop>) -> R
    {
        if CURRENT_LOOP.is_set() {
            CURRENT_LOOP.with(|lp| {
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

    /// Add a new source to an event loop, returning a future which will resolve
    /// to the token that can be used to identify this source.
    ///
    /// When a new I/O object is created it needs to be communicated to the
    /// event loop to ensure that it's registered and ready to receive
    /// notifications. The event loop with then respond with a unique token that
    /// this handle can be identified with (the resolved value of the returned
    /// future).
    ///
    /// This token is then passed in turn to each of the methods below to
    /// interact with notifications on the I/O object itself.
    ///
    /// # Panics
    ///
    /// The returned future will panic if the event loop this handle is
    /// associated with has gone away, or if there is an error communicating
    /// with the event loop.
    pub fn add_source(&self, source: Source) -> AddSource {
        AddSource {
            loop_handle: self.clone(),
            source: Some(source),
            result: None,
        }
    }

    fn add_source_(&self, source: Source, slot: Arc<Slot<io::Result<usize>>>) {
        self.send(Message::AddSource(source, slot));
    }

    /// Begin listening for events on an event loop.
    ///
    /// Once an I/O object has been registered with the event loop through the
    /// `add_source` method, this method can be used with the assigned token to
    /// begin awaiting notifications.
    ///
    /// The `dir` argument indicates how the I/O object is expected to be
    /// awaited on (either readable or writable) and the `wake` callback will be
    /// invoked. Note that one the `wake` callback is invoked once it will not
    /// be invoked again, it must be re-`schedule`d to continue receiving
    /// notifications.
    ///
    /// # Panics
    ///
    /// This function will panic if the event loop this handle is associated
    /// with has gone away, or if there is an error communicating with the event
    /// loop.
    pub fn schedule(&self, tok: usize, dir: Direction, wake: &Arc<Wake>) {
        self.send(Message::Schedule(tok, dir, wake.clone()));
    }

    /// Stop listening for events on an event loop.
    ///
    /// Once a callback has been scheduled with the `schedule` method, it can be
    /// unregistered from the event loop with this method. This method does not
    /// guarantee that the callback will not be invoked if it hasn't already,
    /// but a best effort will be made to ensure it is not called.
    ///
    /// # Panics
    ///
    /// This function will panic if the event loop this handle is associated
    /// with has gone away, or if there is an error communicating with the event
    /// loop.
    pub fn deschedule(&self, tok: usize, dir: Direction) {
        self.send(Message::Deschedule(tok, dir));
    }

    /// Unregister all information associated with a token on an event loop,
    /// deallocating all internal resources assigned to the given token.
    ///
    /// This method should be called whenever a source of events is being
    /// destroyed. This will ensure that the event loop can reuse `tok` for
    /// another I/O object if necessary and also remove it from any poll
    /// notifications and callbacks.
    ///
    /// Note that wake callbacks may still be invoked after this method is
    /// called as it may take some time for the message to drop a source to
    /// reach the event loop. Despite this fact, this method will attempt to
    /// ensure that the callbacks are **not** invoked, so pending scheduled
    /// callbacks cannot be relied upon to get called.
    ///
    /// # Panics
    ///
    /// This function will panic if the event loop this handle is associated
    /// with has gone away, or if there is an error communicating with the event
    /// loop.
    pub fn drop_source(&self, tok: usize) {
        self.send(Message::DropSource(tok));
    }

    /// Send a message to the associated event loop that it should shut down, or
    /// otherwise break out of its current loop of iteration.
    ///
    /// This method does not forcibly cause the event loop to shut down or
    /// perform an interrupt on whatever task is currently running, instead a
    /// message is simply enqueued to at a later date process the request to
    /// stop looping ASAP.
    ///
    /// # Panics
    ///
    /// This function will panic if the event loop this handle is associated
    /// with has gone away, or if there is an error communicating with the event
    /// loop.
    pub fn shutdown(&self) {
        self.send(Message::Shutdown);
    }
}

const ADD_SOURCE_TOKEN: usize = 0;

/// A future which will resolve a unique `tok` token for an I/O object.
///
/// Created through the `LoopHandle::add_source` method, this future can also
/// resolve to an error if there's an issue communicating with the event loop.
pub struct AddSource {
    loop_handle: LoopHandle,
    source: Option<Source>,
    result: Option<(Arc<Slot<io::Result<usize>>>, slot::Token)>,
}

impl Future for AddSource {
    type Item = usize;
    type Error = io::Error;

    fn poll(&mut self, tokens: &Tokens) -> Poll<usize, io::Error> {
        match self.result {
            Some((ref result, _)) => {
                if tokens.may_contain(ADD_SOURCE_TOKEN) {
                    match result.try_consume() {
                        Ok(t) => t.into(),
                        Err(_) => Poll::NotReady,
                    }
                } else {
                    Poll::NotReady
                }
            }
            None => {
                let source = &mut self.source;
                self.loop_handle.with_loop(|lp| {
                    match lp {
                        Some(lp) => lp.add_source(source.take().unwrap()).into(),
                        None => Poll::NotReady,
                    }
                })
            }
        }
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        let wake = wake.clone();
        if let Some((ref result, ref mut token)) = self.result {
            result.cancel(*token);
            *token = result.on_full(move |_| {
                wake.wake(&Tokens::one(ADD_SOURCE_TOKEN));
            });
            return
        }

        let result = Arc::new(Slot::new(None));
        let token = result.on_full(move |_| {
            wake.wake(&Tokens::one(ADD_SOURCE_TOKEN));
        });
        self.result = Some((result.clone(), token));
        self.loop_handle.add_source_(self.source.take().unwrap(), result);
    }
}
