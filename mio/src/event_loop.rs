use std::cell::{Cell, RefCell};
use std::io::{self, ErrorKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};
use std::sync::mpsc;

use mio;
use slab::Slab;
use futures::{Future, Tokens, Wake};

pub type Source = Arc<mio::Evented + Send + Sync>;

static NEXT_LOOP_ID: AtomicUsize = ATOMIC_USIZE_INIT;
scoped_thread_local!(static CURRENT_LOOP: Loop);

const SLAB_CAPACITY: usize = 1024 * 64;

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

#[derive(Copy, Clone)]
pub enum Direction {
    Read,
    Write,
}

struct Scheduled {
    source: Source,
    reader: Option<Arc<Wake>>,
    writer: Option<Arc<Wake>>,
}

impl Scheduled {
    fn waiter_for(&mut self, dir: Direction) -> &mut Option<Arc<Wake>> {
        match dir {
            Direction::Read => &mut self.reader,
            Direction::Write => &mut self.writer,
        }
    }

    fn event_set(&self) -> mio::EventSet {
        let mut set = mio::EventSet::none();
        if self.reader.is_some() {
            set = set | mio::EventSet::readable()
        }
        if self.writer.is_some() {
            set = set | mio::EventSet::writable()
        }
        set
    }
}

enum Message {
    AddSource(Source, Arc<AtomicUsize>, Arc<Wake>),
    DropSource(usize),
    Schedule(usize, Direction, Arc<Wake>),
    Deschedule(usize, Direction),
    Shutdown,
}

fn register(poll: &mut mio::Poll, token: usize, sched: &Scheduled) {
    // TODO: handle error
    poll.register(&*sched.source,
                  mio::Token(token),
                  mio::EventSet::none(),
                  mio::PollOpt::level())
        .unwrap();
}

fn reregister(poll: &mut mio::Poll, token: usize, sched: &Scheduled) {
    // TODO: handle error
    poll.reregister(&*sched.source,
                    mio::Token(token),
                    sched.event_set(),
                    mio::PollOpt::edge() | mio::PollOpt::oneshot())
        .unwrap();
}

fn deregister(poll: &mut mio::Poll, sched: &Scheduled) {
    // TODO: handle error
    poll.deregister(&*sched.source).unwrap();
}

impl Loop {
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

    pub fn handle(&self) -> LoopHandle {
        LoopHandle {
            id: self.id,
            tx: self.tx.clone(),
        }
    }

    pub fn run<F: Future>(self, f: F) -> Result<F::Item, F::Error> {
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

            // TODO: coalesce token sets for a given Wake?
            for i in 0..amt {
                let event = self.io.borrow_mut().events().get(i).unwrap();
                let token = event.token().as_usize();
                if token == 0 {
                    self.consume_queue();
                } else {
                    let mut reader = None;
                    let mut writer = None;

                    if let Some(sched) = self.dispatch.borrow_mut().get_mut(token) {
                        if event.kind().is_readable() {
                            reader = sched.reader.take();
                        }

                        if event.kind().is_writable() {
                            writer = sched.writer.take();
                        }
                    }

                    CURRENT_LOOP.set(&self, || {
                        if let Some(reader_wake) = reader.take() {
                            reader_wake.wake(&Tokens::from_usize(token));
                        }
                        if let Some(writer_wake) = writer.take() {
                            writer_wake.wake(&Tokens::from_usize(token));
                        }
                    });

                    // For now, always reregister, to deal with the fact that
                    // combined oneshot + read|write requires rearming even if
                    // only one side fired.
                    //
                    // TODO: optimize this
                    if let Some(sched) = self.dispatch.borrow().get(token) {
                        reregister(&mut self.io.borrow_mut(), token, &sched);
                    }
                }
            }
        }

        rx_res.recv().unwrap()
    }

    fn add_source(&self, source: Source) -> usize {
        let sched = Scheduled {
            source: source,
            reader: None,
            writer: None,
        };
        let mut dispatch = self.dispatch.borrow_mut();
        // TODO: handle out of space
        let entry = dispatch.vacant_entry().unwrap();
        register(&mut self.io.borrow_mut(), entry.index(), &sched);
        entry.insert(sched).index()
    }

    fn drop_source(&self, token: usize) {
        let sched = self.dispatch.borrow_mut().remove(token).unwrap();
        deregister(&mut self.io.borrow_mut(), &sched);
    }

    fn schedule(&self, token: usize, dir: Direction, wake: Arc<Wake>) {
        let mut dispatch = self.dispatch.borrow_mut();
        let sched = dispatch.get_mut(token).unwrap();
        *sched.waiter_for(dir) = Some(wake);
        reregister(&mut self.io.borrow_mut(), token, sched);
    }

    fn deschedule(&self, token: usize, dir: Direction) {
        let mut dispatch = self.dispatch.borrow_mut();
        let sched = dispatch.get_mut(token).unwrap();
        *sched.waiter_for(dir) = None;
        reregister(&mut self.io.borrow_mut(), token, sched);
    }

    fn consume_queue(&self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.notify(msg);
        }
    }

    fn notify(&self, msg: Message) {
        match msg {
            Message::AddSource(source, id, wake) => {
                let tok = self.add_source(source);
                id.store(tok, Ordering::Relaxed);
                wake.wake(&Tokens::from_usize(ADD_SOURCE_TOKEN));
            }
            Message::DropSource(tok) => self.drop_source(tok),
            Message::Schedule(tok, dir, wake) => self.schedule(tok, dir, wake),
            Message::Deschedule(tok, dir) => self.deschedule(tok, dir),
            Message::Shutdown => self.active.set(false),
        }
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
                    // TODO: handle failure
                    self.tx
                        .send(msg)
                        .map_err(|_| ())
                        .expect("failed to send register message")
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
    pub fn add_source(&self, source: Source) -> AddSource {
        AddSource {
            loop_handle: self.clone(),
            source: Some(source),
            id: Arc::new(AtomicUsize::new(0)),
            scheduled: false,
        }
    }

    fn add_source_(&self, source: Source, id: Arc<AtomicUsize>, wake: Arc<Wake>) {
        self.send(Message::AddSource(source, id, wake));
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
    pub fn schedule(&self, tok: usize, dir: Direction, wake: Arc<Wake>) {
        self.send(Message::Schedule(tok, dir, wake));
    }

    /// Stop listening for events on an event loop.
    ///
    /// Once a callback has been scheduled with the `schedule` method, it can be
    /// unregistered from the event loop with this method. This method does not
    /// guarantee that the callback will not be invoked if it hasn't already,
    /// but a best effort will be made to ensure it is not called.
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
    pub fn drop_source(&self, tok: usize) {
        self.send(Message::DropSource(tok));
    }

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
    id: Arc<AtomicUsize>,
    scheduled: bool,
}

impl Future for AddSource {
    type Item = usize;
    type Error = io::Error; // TODO: integrate channel error?

    fn poll(&mut self, tokens: &Tokens) -> Option<Result<usize, io::Error>> {
        if self.scheduled {
            if tokens.may_contain(&Tokens::from_usize(ADD_SOURCE_TOKEN)) {
                let id = self.id.load(Ordering::Relaxed);
                if id != 0 {
                    return Some(Ok(id))
                }
            }
        } else {
            if CURRENT_LOOP.is_set() {
                let res = CURRENT_LOOP.with(|lp| {
                    if lp.id == self.loop_handle.id {
                        Some(lp.add_source(self.source.take().unwrap()))
                    } else {
                        None
                    }
                });
                if let Some(id) = res {
                    return Some(Ok(id));
                }
            }
        }

        None
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        if self.scheduled { return; }
        self.scheduled = true;
        self.loop_handle.add_source_(self.source.take().unwrap(), self.id.clone(), wake);
    }
}
