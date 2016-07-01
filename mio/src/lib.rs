extern crate mio;
extern crate futures;
extern crate fnv;

use std::hash::BuildHasherDefault;
use std::collections::HashMap;
use std::io::{self, ErrorKind, Read, Write};
use std::net::SocketAddr;
use std::panic;
use std::slice;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, TryRecvError};

use fnv::FnvHasher;

use futures::{Future, Tokens, promise, Complete, Wake, PollError, PollResult};

pub struct MioEvent {
    events: mio::EventSet,
    source: Source,
    loop_handle: LoopHandle,
}

impl MioEvent {
    fn into_future<C: PollCompletion>(self, completion: C) -> MioFuture<C> {
        MioFuture {
            event: self,
            completion: completion,
        }
    }
}

pub trait PollCompletion {
    type Item: Send + 'static;
    type Error: Send + 'static;

    fn poll_completion(&mut self) -> Option<PollResult<Self::Item, Self::Error>>;
}

pub struct MioFuture<C> {
    event: MioEvent,
    completion: C,
}

impl<C> Future for MioFuture<C> where C: PollCompletion + Send + 'static {
    type Item = C::Item;
    type Error = C::Error;

    fn poll(&mut self, tokens: &Tokens) -> Option<PollResult<C::Item, C::Error>> {
        // TODO: filter by tokens
        self.completion.poll_completion()
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        // TODO: record token
        self.event.loop_handle.schedule(Interest {
            waiter: wake,
            source: self.event.source.clone(),
            events: self.event.events,
            first_time: false, // assume already registered
        });
    }

    fn tailcall(&mut self) -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        None
    }
}


#[derive(Clone)]
pub struct TcpListener {
    tcp: Arc<mio::tcp::TcpListener>,
    loop_handle: LoopHandle,
}

pub struct TcpListenerAccept {
    inner: TcpListener
}

impl TcpListener {
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp.local_addr()
    }

    pub fn accept(&self) -> TcpListenerAccept {
        TcpListenerAccept { inner: self.clone() }
    }
}

impl Future for TcpListenerAccept {
    type Item = (TcpStream, SocketAddr);
    type Error = io::Error;

    fn poll(&mut self, tokens: &Tokens) -> Option<PollResult<Self::Item, Self::Error>> {
        // TODO: attempt poll only if tokens match
        match self.inner.tcp.accept() {
            Err(e) => Some(Err(PollError::Other(e))),
            Ok(Some((tcp, addr))) => {
                let tcp = TcpStream {
                    tcp: Arc::new(tcp),
                    loop_handle: self.inner.loop_handle.clone(),
                };
                self.inner.loop_handle.register(tcp.tcp.clone());
                Some(Ok((tcp, addr)))
            }
            Ok(None) => None
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        // TODO: record token for dtor
        self.inner.loop_handle.schedule(Interest {
            waiter: wake,
            source: self.inner.tcp.clone(),
            events: mio::EventSet::readable(),
            first_time: false,
        });
    }

    fn tailcall(&mut self) -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        None
    }
}

#[derive(Clone)]
pub struct TcpStream {
    tcp: Arc<mio::tcp::TcpStream>,
    loop_handle: LoopHandle,
}

pub struct ReadCompletion {
    tcp: Arc<mio::tcp::TcpStream>,
    into: Option<Vec<u8>>,
}

impl PollCompletion for ReadCompletion {
    type Item = Vec<u8>;
    type Error = Error<Vec<u8>>;

    fn poll_completion(&mut self) -> Option<PollResult<Self::Item, Self::Error>> {
        let mut into = self.into.take().unwrap();
        let r = unsafe {
            (&*self.tcp).read(slice_to_end(&mut into))
        };
        match r {
            Ok(i) => {
                unsafe {
                    let len = into.len();
                    into.set_len(len + i);
                }
                Some(Ok(into))
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    self.into = Some(into);
                    None
                } else {
                    Some(Err(PollError::Other(Error::new(e, into))))
                }
            }
        }
    }
}

pub struct WriteCompletion {
    tcp: Arc<mio::tcp::TcpStream>,
    offset: usize,
    data: Option<Vec<u8>>,
}

impl PollCompletion for WriteCompletion {
    type Item = (usize, Vec<u8>);
    type Error = Error<(usize, Vec<u8>)>;

    fn poll_completion(&mut self) -> Option<PollResult<Self::Item, Self::Error>> {
        let mut data = self.data.take().unwrap();
        let r = (&*self.tcp).write(&data[self.offset..]);
        match r {
            Ok(i) => Some(Ok((self.offset + i, data))),
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    self.data = Some(data);
                    None
                } else {
                    Some(Err(PollError::Other(Error::new(e, (self.offset, data)))))
                }
            }
        }
    }
}

unsafe fn slice_to_end(v: &mut Vec<u8>) -> &mut [u8] {
    slice::from_raw_parts_mut(v.as_mut_ptr().offset(v.len() as isize),
                              v.capacity() - v.len())
}

impl TcpStream {
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp.local_addr()
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.tcp.peer_addr()
    }

    pub fn ready_to_read(&self) -> MioEvent {
        MioEvent {
            events: mio::EventSet::readable(),
            source: self.tcp.clone(),
            loop_handle: self.loop_handle.clone(),
        }
    }

    pub fn ready_to_write(&self) -> MioEvent {
        MioEvent {
            events: mio::EventSet::writable(),
            source: self.tcp.clone(),
            loop_handle: self.loop_handle.clone(),
        }
    }

    // TODO: wrap in newtype
    pub fn read(&self, mut into: Vec<u8>) -> MioFuture<ReadCompletion> {
        self.ready_to_read().into_future(ReadCompletion {
            tcp: self.tcp.clone(),
            into: Some(into)
        })
    }

    pub fn write(&self, offset: usize, data: Vec<u8>) -> MioFuture<WriteCompletion> {
        self.ready_to_write().into_future(WriteCompletion {
            tcp: self.tcp.clone(),
            offset: offset,
            data: Some(data),
        })
    }
}

type Waiter = Arc<Wake>;
type Source = Arc<mio::Evented + Send + Sync>;

pub struct Loop {
    io: mio::Poll,
    tx: mio::channel::Sender<Message>,
    rx: mio::channel::Receiver<Message>,
    dispatch: HashMap<usize, Waiter, BuildHasherDefault<FnvHasher>>,
    token_counter: TokenCounter,
}

#[derive(Clone)]
pub struct LoopHandle {
    tx: mio::channel::Sender<Message>,
    tok: TokenCounter,
}

#[derive(Clone)]
struct TokenCounter {
    counter: Arc<AtomicUsize>
}

struct Interest {
    waiter: Waiter,
    source: Source,
    events: mio::EventSet,
    first_time: bool,
}

enum Message {
    Register(Source),
    Schedule(usize, Interest),
    Deschedule(usize),
    Shutdown
}

impl Loop {
    pub fn new() -> io::Result<Loop> {
        let (tx, rx) = mio::channel::from_std_channel(channel());
        let io = try!(mio::Poll::new());
        try!(io.register(&rx,
                         mio::Token(0),
                         mio::EventSet::readable(),
                         mio::PollOpt::edge()));
        Ok(Loop {
            io: io,
            tx: tx,
            rx: rx,
            dispatch: HashMap::default(),
            token_counter: TokenCounter::new(),
        })
    }

    pub fn run(&mut self) {
        loop {
            let amt;
            // On Linux, Poll::poll is epoll_wait, which may return EINTR if a
            // ptracer attaches. This retry loop prevents crashing when
            // attaching strace, or similar.
            loop {
                match self.io.poll(None) {
                    Ok(a) => {
                        amt = a;
                        break;
                    }
                    Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                    err@Err(_) => { err.unwrap(); },
                }
            }

            // TODO: attempt to coalesce events into token sets when they are
            // for the same Wake
            for i in 0..amt {
                let event = self.io.events().get(i).unwrap();
                let token = event.token().as_usize();
                if token == 0 {
                    while let Ok(msg) = self.rx.try_recv() {
                        self.notify(msg);
                    }
                } else if let Some(wake) = self.dispatch.remove(&token) {
                    wake.wake(&Tokens::from_usize(token));
                }
            }
        }
    }

    fn register_(&mut self, source: Source) {
        self.io.register(&*source, mio::Token(0), mio::EventSet::none(), mio::PollOpt::empty());
    }

    fn schedule_(&mut self, token: usize, interest: Interest) {
        let Interest { waiter, source, events, first_time } = interest;
        let old = self.dispatch.insert(token, waiter);
        debug_assert!(old.is_none());

        // TODO handle failure
        if first_time {
            self.io.register(&*source,
                             mio::Token(token),
                             events,
                             mio::PollOpt::edge() | mio::PollOpt::oneshot()).unwrap();
        } else {
            self.io.reregister(&*source,
                               mio::Token(token),
                               events,
                               mio::PollOpt::edge() | mio::PollOpt::oneshot()).unwrap();
        }
    }

    fn deschedule_(&mut self, token: usize) {
        self.dispatch.remove(&token);
    }

    fn notify(&mut self, msg: Message) {
        match msg {
            Message::Register(source) => self.register_(source),
            Message::Schedule(token, interest) => self.schedule_(token, interest),
            Message::Deschedule(tok) => self.deschedule_(tok),
            Message::Shutdown => unimplemented!()
        }
    }

    fn handle(&self) -> LoopHandle {
        LoopHandle {
            tx: self.tx.clone(),
            tok: self.token_counter.clone(),
        }
    }

/*
    pub fn tcp_connect(&mut self, addr: &SocketAddr)
                       -> Box<IoFuture<TcpStream>> {
        let pair = mio::tcp::TcpStream::connect(addr).and_then(|tcp| {
            let token = self.next;
            self.next += 1;
            try!(self.io.register(&tcp,
                                  mio::Token(token),
                                  mio::EventSet::writable(),
                                  mio::PollOpt::edge() |
                                    mio::PollOpt::oneshot()));
            Ok((tcp, token))
        });
        match pair {
            Ok((tcp, token)) => {
                let (p, c) = promise();
                assert!(self.done.insert(token, c).is_none());
                let tx = self.tx.clone();
                p.map(|()| {
                    TcpStream {
                        tcp: Arc::new(tcp),
                        tx: tx,
                    }
                }).boxed()
            }
            Err(e) => futures::failed(e).boxed(),
        }
    }
*/

    pub fn tcp_listen(&mut self, addr: &SocketAddr) -> io::Result<TcpListener> {
        let tcp = try!(mio::tcp::TcpListener::bind(addr));

        // dummy registration, so that we can always re-register in the future
        try!(self.io.register(&tcp,
                              mio::Token(0),
                              mio::EventSet::none(),
                              mio::PollOpt::empty()));

        Ok(TcpListener {
            tcp: Arc::new(tcp),
            loop_handle: self.handle(),
        })
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

// TODO: use TLS to avoid sending messages
impl LoopHandle {
    fn register(&self, source: Source) {
        self.tx.send(Message::Register(source))
            .map_err(|_| ())
            .expect("failed to send register message") // todo: handle failure
    }

    fn schedule(&self, interest: Interest) -> usize {
        let token = self.tok.next_token();
        self.tx.send(Message::Schedule(token, interest))
            .map_err(|_| ())
            .expect("failed to send schedule message"); // TODO: handle failure?
        token
    }

    fn deschedule(&self, token: usize) {
        unimplemented!()
    }
}

pub struct Error<T> {
    err: io::Error,
    data: T,
}

impl<T> Error<T> {
    pub fn new(err: io::Error, data: T) -> Error<T> {
        Error {
            err: err,
            data: data,
        }
    }

    pub fn into_pair(self) -> (io::Error, T) {
        (self.err, self.data)
    }
}

impl<T> From<Error<T>> for io::Error {
    fn from(e: Error<T>) -> io::Error {
        e.err
    }
}
