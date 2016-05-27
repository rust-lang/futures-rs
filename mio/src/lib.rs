extern crate mio;
extern crate futures;

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::panic;
use std::slice;
use std::sync::Arc;
use std::sync::mpsc::{channel, TryRecvError};

use futures::{Future, promise, Complete, PollError};

pub type IoFuture<T> = Future<Item=T, Error=io::Error>;

pub struct Loop {
    io: mio::Poll,
    tx: mio::channel::Sender<Message>,
    rx: mio::channel::Receiver<Message>,
    next: usize,
    done: HashMap<usize, Complete<(), io::Error>>,
}

enum Message {
    Wait(Complete<(), io::Error>, mio::EventSet, Arc<mio::Evented + Send + Sync>),
    Register(Arc<mio::Evented + Send + Sync>),
}

pub struct TcpListener {
    tcp: Arc<mio::tcp::TcpListener>,
    tx: mio::channel::Sender<Message>,
}

pub struct ErrorBuf {
    buf: Vec<u8>,
    err: io::Error,
}

impl TcpListener {
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp.local_addr()
    }

    pub fn accept(&self) -> Box<IoFuture<(TcpStream, SocketAddr)>> {
        match self.tcp.accept() {
            Err(e) => return futures::failed(e).boxed(),
            Ok(Some((tcp, addr))) => {
                let tcp = TcpStream {
                    tcp: Arc::new(tcp),
                    tx: self.tx.clone(),
                };
                let res = self.tx.send(Message::Register(tcp.tcp.clone()));
                let res = res.map(|()| (tcp, addr));
                let res = res.map_err(|e| {
                    match e {
                        mio::channel::SendError::Io(e) => e,
                        // TODO: need to handle a closed channel
                        mio::channel::SendError::Disconnected(..) => {
                            panic!("closed channel")
                        }
                    }
                });
                return futures::done(res).boxed()
            }
            Ok(None) => {}
        }

        let (p, c) = promise();
        let r = self.tx.send(Message::Wait(c,
                                           mio::EventSet::readable(),
                                           self.tcp.clone()));
        match r {
            Ok(()) => {
                let me = TcpListener {
                    tcp: self.tcp.clone(),
                    tx: self.tx.clone(),
                };
                p.and_then(move |()| me.accept()).boxed()
            }
            Err(mio::channel::SendError::Io(e)) => {
                return futures::failed(e).boxed()
            }
            Err(mio::channel::SendError::Disconnected(..)) => panic!("closed channel"),
        }
    }
}

pub struct TcpStream {
    tcp: Arc<mio::tcp::TcpStream>,
    tx: mio::channel::Sender<Message>,
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

    pub fn read(&self, mut into: Vec<u8>)
                -> Box<Future<Item=Vec<u8>, Error=ErrorBuf>> {
        let r = unsafe {
            (&*self.tcp).read(slice_to_end(&mut into))
        };
        match r {
            Ok(i) => {
                unsafe {
                    let len = into.len();
                    into.set_len(len + i);
                }
                return futures::finished(into).boxed()
            }
            Err(e) => {
                if e.kind() != io::ErrorKind::WouldBlock {
                    return futures::failed(ErrorBuf {
                        err: e,
                        buf: into,
                    }).boxed()
                }
            }
        }
        let (p, c) = promise();
        let r = self.tx.send(Message::Wait(c,
                                           mio::EventSet::readable(),
                                           self.tcp.clone()));
        match r {
            Ok(()) => {
                let me2 = TcpStream {
                    tcp: self.tcp.clone(),
                    tx: self.tx.clone(),
                };
                p.then(move |res| {
                    match res {
                        Ok(()) => me2.read(into),
                        Err(e) => {
                            futures::failed(ErrorBuf {
                                err: e,
                                buf: into,
                            }).boxed()
                        }
                    }
                }).boxed()
            }
            Err(mio::channel::SendError::Io(e)) => {
                return futures::failed(ErrorBuf {
                    err: e,
                    buf: into,
                }).boxed()
            }
            Err(mio::channel::SendError::Disconnected(..)) => panic!("closed channel"),
        }
    }

    pub fn write(&self, offset: usize, data: Vec<u8>)
                 -> Box<Future<Item=(usize, Vec<u8>), Error=ErrorBuf>> {
        let r = (&*self.tcp).write(&data[offset..]);
        match r {
            Ok(i) => return futures::finished((offset + i, data)).boxed(),
            Err(e) => {
                if e.kind() != io::ErrorKind::WouldBlock {
                    return futures::failed(ErrorBuf {
                        buf: data,
                        err: e
                    }).boxed()
                }
            }
        }
        let (p, c) = promise();
        let r = self.tx.send(Message::Wait(c,
                                           mio::EventSet::writable(),
                                           self.tcp.clone()));
        match r {
            Ok(()) => {
                let me2 = TcpStream {
                    tcp: self.tcp.clone(),
                    tx: self.tx.clone(),
                };
                p.then(move |res| {
                    match res {
                        Ok(()) => me2.write(offset, data),
                        Err(e) => {
                            futures::failed(ErrorBuf {
                                buf: data,
                                err: e
                            }).boxed()
                        }
                    }
                }).boxed()
            }
            Err(mio::channel::SendError::Io(e)) => {
                return futures::failed(ErrorBuf {
                    err: e,
                    buf: data,
                }).boxed()
            }
            Err(mio::channel::SendError::Disconnected(..)) => panic!("closed channel"),
        }
    }
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
            done: HashMap::new(),
            next: 1,
            tx: tx,
            rx: rx,
        })
    }

    pub fn await<F: Future>(&mut self, mut f: F)
                            -> Result<F::Item, F::Error> {
        let (tx, rx) = channel();
        f.schedule(move |r| {
            drop(tx.send(r))
            // TODO: signal to the event loop that it should wake up
        });
        let mut ret = None;
        self._await(&mut || {
            match rx.try_recv() {
                Ok(e) => ret = Some(e),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => panic!(),
            }
            ret.is_some()
        });
        match ret.unwrap() {
            Ok(e) => Ok(e),
            Err(PollError::Other(e)) => Err(e),
            Err(PollError::Panicked(p)) => panic::resume_unwind(p),
            Err(PollError::Canceled) => panic!("canceled"),
        }
    }

    fn _await(&mut self, done: &mut FnMut() -> bool) {
        while !done() {
            let amt = self.io.poll(None).unwrap();

            for i in 0..amt {
                let event = self.io.events().get(i).unwrap();
                let token = event.token().as_usize();
                if token == 0 {
                    while let Ok(msg) = self.rx.try_recv() {
                        self.notify(msg);
                    }
                } else if let Some(complete) = self.done.remove(&token) {
                    complete.finish(());
                }
            }
        }
    }

    fn notify(&mut self, msg: Message) {
        match msg {
            Message::Wait(c, events, evented) => {
                let token = self.next;
                self.next += 1;
                let evented: &mio::Evented = &*evented;
                let r = self.io.reregister(evented,
                                           mio::Token(token),
                                           events,
                                           mio::PollOpt::edge() |
                                              mio::PollOpt::oneshot());
                match r {
                    Ok(()) => {
                        self.done.insert(token, c);
                    }
                    Err(e) => c.fail(e),
                }
            }
            Message::Register(evented) => {
                // TODO: propagate this error somewhere
                let evented: &mio::Evented = &*evented;
                self.io.register(evented,
                                 mio::Token(0),
                                 mio::EventSet::none(),
                                 mio::PollOpt::empty()).unwrap();
            }
        }
    }

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

    pub fn tcp_listen(&mut self, addr: &SocketAddr) -> io::Result<TcpListener> {
        let tcp = try!(mio::tcp::TcpListener::bind(addr));
        try!(self.io.register(&tcp,
                              mio::Token(0),
                              mio::EventSet::none(),
                              mio::PollOpt::empty()));

        Ok(TcpListener {
            tcp: Arc::new(tcp),
            tx: self.tx.clone(),
        })
    }
}

impl ErrorBuf {
    pub fn into_pair(self) -> (io::Error, Vec<u8>) {
        (self.err, self.buf)
    }
}

impl From<ErrorBuf> for io::Error {
    fn from(buf: ErrorBuf) -> io::Error {
        buf.err
    }
}
