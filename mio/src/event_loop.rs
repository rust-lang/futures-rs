use std::cell::{Cell, RefCell};
use std::io::{self, ErrorKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};
use std::sync::mpsc;

use mio;
use slab::Slab;
use futures::{Future, Tokens, Wake, PollResult};

pub type Waiter = Arc<Wake>;
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
    reader: Option<Waiter>,
    writer: Option<Waiter>,
}

impl Scheduled {
    fn waiter_for(&mut self, dir: Direction) -> &mut Option<Waiter> {
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
    AddSource(Source, Arc<AtomicUsize>, Waiter),
    DropSource(usize),
    Schedule(usize, Direction, Waiter),
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

    fn schedule(&self, token: usize, dir: Direction, wake: Waiter) {
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
        let mut msg_dance = Some(msg);

        if CURRENT_LOOP.is_set() {
            CURRENT_LOOP.with(|lp| {
                if lp.id == self.id {
                    // Need to execute all existing requests first, to ensure
                    // that our message is processed "in order"
                    lp.consume_queue();
                    lp.notify(msg_dance.take().unwrap());
                }
            })
        }

        if let Some(msg) = msg_dance.take() {
            self.tx
                .send(msg)
                .map_err(|_| ())
                .expect("failed to send register message") // todo: handle failure
        }
    }

    pub fn add_source(&self, source: Source) -> AddSource {
        AddSource {
            loop_handle: self.clone(),
            source: Some(source),
            id: Arc::new(AtomicUsize::new(0)),
            scheduled: false,
        }
    }

    fn add_source_(&self, source: Source, id: Arc<AtomicUsize>, wake: Waiter) {
        self.send(Message::AddSource(source, id, wake));
    }

    pub fn drop_source(&self, tok: usize) {
        self.send(Message::DropSource(tok));
    }

    pub fn schedule(&self, tok: usize, dir: Direction, wake: Waiter) {
        self.send(Message::Schedule(tok, dir, wake));
    }

    pub fn deschedule(&self, tok: usize, dir: Direction) {
        self.send(Message::Deschedule(tok, dir));
    }

    pub fn shutdown(&self) {
        self.send(Message::Shutdown);
    }
}

const ADD_SOURCE_TOKEN: usize = 0;

pub struct AddSource {
    loop_handle: LoopHandle,
    source: Option<Source>,
    id: Arc<AtomicUsize>,
    scheduled: bool,
}

impl Future for AddSource {
    type Item = usize;
    type Error = io::Error; // TODO: integrate channel error?

    fn poll(&mut self, tokens: &Tokens) -> Option<PollResult<usize, io::Error>> {
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
