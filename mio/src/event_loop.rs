use std::cell::RefCell;
use std::hash::BuildHasherDefault;
use std::collections::HashMap;
use std::io::{self, ErrorKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};
use std::sync::mpsc;

use mio;
use fnv::FnvHasher;
use futures::{Tokens, Wake};

pub type Waiter = Arc<Wake>;
pub type Source = Arc<mio::Evented + Send + Sync>;

static NEXT_LOOP_ID: AtomicUsize = ATOMIC_USIZE_INIT;

scoped_thread_local!(static CURRENT_LOOP: Loop);

pub struct Loop {
    id: usize,
    io: RefCell<mio::Poll>,
    tx: mio::channel::Sender<Message>,
    rx: mio::channel::Receiver<Message>,
    counter: TokenCounter,
    dispatch: RefCell<HashMap<usize, Scheduled, BuildHasherDefault<FnvHasher>>>,
}

#[derive(Clone)]
pub struct LoopHandle {
    id: usize,
    tx: mio::channel::Sender<Message>,
    counter: TokenCounter,
}

#[derive(Clone)]
struct TokenCounter {
    counter: Arc<AtomicUsize>,
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
    AddSource(usize, Source),
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
            io: RefCell::new(io),
            tx: tx,
            rx: rx,
            dispatch: RefCell::new(HashMap::default()),
            counter: TokenCounter::new(),
        })
    }

    pub fn handle(&self) -> LoopHandle {
        LoopHandle {
            id: self.id,
            counter: self.counter.clone(),
            tx: self.tx.clone(),
        }
    }

    pub fn run(&mut self) {
        loop {
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
                    while let Ok(msg) = self.rx.try_recv() {
                        self.notify(msg);
                    }
                } else {
                    let mut reader = None;
                    let mut writer = None;

                    if let Some(sched) = self.dispatch.borrow_mut().get_mut(&token) {
                        if event.kind().is_readable() {
                            reader = sched.reader.take();
                        }

                        if event.kind().is_writable() {
                            writer = sched.writer.take();
                        }
                    }

                    CURRENT_LOOP.set(self, || {
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
                    if let Some(sched) = self.dispatch.borrow().get(&token) {
                        reregister(&mut self.io.borrow_mut(), token, &sched);
                    }
                }
            }
        }
    }

    fn add_source(&self, token: usize, source: Source) {
        let sched = Scheduled {
            source: source,
            reader: None,
            writer: None,
        };
        register(&mut self.io.borrow_mut(), token, &sched);
        let old = self.dispatch.borrow_mut().insert(token, sched);
        debug_assert!(old.is_none());
    }

    fn drop_source(&self, token: usize) {
        let sched = self.dispatch.borrow_mut().remove(&token).unwrap();
        deregister(&mut self.io.borrow_mut(), &sched);
    }

    fn schedule(&self, token: usize, dir: Direction, wake: Waiter) {
        let mut dispatch = self.dispatch.borrow_mut();
        let sched = dispatch.get_mut(&token).unwrap();
        *sched.waiter_for(dir) = Some(wake);
        reregister(&mut self.io.borrow_mut(), token, sched);
    }

    fn deschedule(&self, token: usize, dir: Direction) {
        let mut dispatch = self.dispatch.borrow_mut();
        let sched = dispatch.get_mut(&token).unwrap();
        *sched.waiter_for(dir) = None;
        reregister(&mut self.io.borrow_mut(), token, sched);
    }

    fn notify(&self, msg: Message) {
        match msg {
            Message::AddSource(tok, source) => self.add_source(tok, source),
            Message::DropSource(tok) => self.drop_source(tok),
            Message::Schedule(tok, dir, wake) => self.schedule(tok, dir, wake),
            Message::Deschedule(tok, dir) => self.deschedule(tok, dir),
            Message::Shutdown => unimplemented!(),
        }
    }
}

impl LoopHandle {
    fn send(&self, msg: Message) {
        let mut msg_dance = Some(msg);

        if CURRENT_LOOP.is_set() {
            CURRENT_LOOP.with(|lp| {
                if lp.id == self.id {
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

    pub fn add_source(&self, source: Source) -> usize {
        let tok = self.counter.next_token();
        self.send(Message::AddSource(tok, source));
        tok
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
}

impl TokenCounter {
    pub fn new() -> TokenCounter {
        TokenCounter { counter: Arc::new(AtomicUsize::new(1)) }
    }

    pub fn next_token(&self) -> usize {
        // TODO: handle rollover robustly...
        // the 0 token is reserved
        let mut next = 0;
        while next == 0 {
            next = self.counter.fetch_add(1, Ordering::Relaxed);
        }
        next
    }
}
