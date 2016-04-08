extern crate mio;
extern crate futures;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::io;

// use self::mio::{TryRead, TryWrite};
//
use futures::{Future, FutureResult, promise, Complete};
// use slot::Slot;
//
// thread_local!{
//     pub static INNER: Arc<Inner> = Arc::new(Inner {
//         poll: mio::Poll::new().unwrap(),
//     })
// }

pub type IoFuture<T> = Future<Item=T, Error=io::Error>;

pub struct Loop {
    io: mio::EventLoop<Inner>,
    inner: Inner,
}

struct Inner {
    next: usize,
    done: HashMap<usize, Complete<(), io::Error>>,
}

enum Message {
    Wait(Complete<(), io::Error>),
}

// pub struct TcpConnect {
//     tcp: mio::tcp::TcpStream,
//     slot: Arc<Slot<mio::EventSet>>,
//     inner: Arc<Inner>,
// }
//
// impl Future for TcpConnect {
//     type Item = TcpStream;
//     type Error = io::Error;
//
//     fn poll(self) -> Result<io::Result<TcpStream>, TcpConnect> {
//         match self.slot.try_consume() {
//             Ok(_events) => Ok(Ok(TcpStream::new(self.tcp, self.inner))),
//             Err(..) => Err(self),
//         }
//     }
//
//     fn schedule<G>(self, g: G)
//         where G: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static
//     {
//         self.slot.clone().on_full(move |_events| {
//             g(Ok(TcpStream::new(self.tcp, self.inner)))
//         });
//     }
// }
//
pub struct TcpListener {
    tcp: mio::tcp::TcpListener,
    // slot: Arc<Slot<mio::EventSet>>,
    io: mio::Sender<Message>,
}

impl TcpListener {
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp.local_addr()
    }

    pub fn accept(&self) -> Box<IoFuture<TcpStream>> {
        let (p, c) = promise();
        drop(c);
        p.boxed()
    }
}

// impl Future for TcpListener {
//     type Item = (TcpStream, SocketAddr, TcpListener);
//     type Error = io::Error;
//
//     fn poll(self) -> Result<io::Result<(TcpStream, SocketAddr, TcpListener)>,
//                                        TcpListener> {
//         match self.tcp.accept() {
//             Ok(Some((stream, addr))) => {
//                 let stream = TcpStream::new(stream, self.inner.clone());
//                 Ok(Ok((stream, addr, self)))
//             }
//             Ok(None) => Err(self),
//             Err(e) => Ok(Err(e)),
//         }
//     }
//
//     fn schedule<G>(self, g: G)
//         where G: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static
//     {
//         let me = match self.poll() {
//             Ok(item) => return g(item),
//             Err(me) => me,
//         };
//         let res = me.inner.poll.register(&me.tcp,
//                                          slot2token(me.slot.clone()),
//                                          mio::EventSet::readable(),
//                                          mio::PollOpt::edge() |
//                                              mio::PollOpt::oneshot());
//         if let Err(e) = res {
//             return g(Err(e))
//         }
//         me.slot.clone().on_full(move |slot| {
//             slot.try_consume().ok().unwrap();
//             me.schedule(g)
//         });
//     }
// }
//
pub struct TcpStream {
    tcp: mio::tcp::TcpStream,
    io: mio::Sender<Message>,
}

impl TcpStream {
//     fn new(tcp: mio::tcp::TcpStream, inner: Arc<Inner>) -> TcpStream {
//         TcpStream {
//             tcp: tcp,
//             inner: inner,
//         }
//     }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp.local_addr()
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.tcp.peer_addr()
    }

    // TODO: give back the buffer
    pub fn read(&self, _into: Vec<u8>) -> Box<IoFuture<Vec<u8>>> {
        loop {}
        // let slot = Arc::new(Slot::new(None));
        // Ok(TcpRead {
        //     stream: self,
        //     buf: into,
        //     slot: slot,
        // })
    }

//     pub fn write(self, buf: Vec<u8>) -> io::Result<TcpWrite> {
//         let slot = Arc::new(Slot::new(None));
//         Ok(TcpWrite {
//             stream: self,
//             buf: buf,
//             slot: slot,
//         })
//     }
//
//     fn try_read(&self, into: &mut Vec<u8>) -> io::Result<Option<usize>> {
//         let mut tcp = &self.tcp;
//         unsafe {
//             let cur = into.len();
//             let dst = into.as_mut_ptr().offset(cur as isize);
//             let len = into.capacity() - cur;
//             match tcp.try_read(slice::from_raw_parts_mut(dst, len)) {
//                 Ok(Some(amt)) => {
//                     into.set_len(cur + amt);
//                     Ok(Some(amt))
//                 }
//                 other => other,
//             }
//         }
//     }
}
//
// pub struct TcpRead {
//     stream: TcpStream,
//     buf: Vec<u8>,
//     slot: Arc<Slot<mio::EventSet>>,
// }
//
// impl Future for TcpRead {
//     type Item = (Vec<u8>, usize, TcpStream);
//     type Error = io::Error;
//
//     fn poll(mut self) -> Result<io::Result<Self::Item>, Self> {
//         match self.stream.try_read(&mut self.buf) {
//             Ok(Some(amt)) => Ok(Ok((self.buf, amt, self.stream))),
//             Ok(None) => Err(self),
//             Err(e) => Ok(Err(e)),
//         }
//     }
//
//     fn schedule<G>(self, g: G)
//         where G: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static
//     {
//         let me = match self.poll() {
//             Ok(item) => return g(item),
//             Err(me) => me,
//         };
//         let res = me.stream.inner.poll.register(&me.stream.tcp,
//                                                 slot2token(me.slot.clone()),
//                                                 mio::EventSet::readable(),
//                                                 mio::PollOpt::edge() |
//                                                         mio::PollOpt::oneshot());
//         if let Err(e) = res {
//             return g(Err(e))
//         }
//         me.slot.clone().on_full(move |slot| {
//             slot.try_consume().ok().unwrap();
//             me.schedule(g)
//         });
//     }
// }
//
// pub struct TcpWrite {
//     stream: TcpStream,
//     buf: Vec<u8>,
//     slot: Arc<Slot<mio::EventSet>>,
// }
//
// impl Future for TcpWrite {
//     type Item = (Vec<u8>, usize, TcpStream);
//     type Error = io::Error;
//
//     fn poll(mut self) -> Result<io::Result<Self::Item>, Self> {
//         match (&self.stream.tcp).try_write(&mut self.buf) {
//             Ok(Some(amt)) => Ok(Ok((self.buf, amt, self.stream))),
//             Ok(None) => Err(self),
//             Err(e) => Ok(Err(e)),
//         }
//     }
//
//     fn schedule<G>(self, g: G)
//         where G: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static
//     {
//         let me = match self.poll() {
//             Ok(item) => return g(item),
//             Err(me) => me,
//         };
//         let res = me.stream.inner.poll.register(&me.stream.tcp,
//                                                 slot2token(me.slot.clone()),
//                                                 mio::EventSet::writable(),
//                                                 mio::PollOpt::edge() |
//                                                         mio::PollOpt::oneshot());
//         if let Err(e) = res {
//             return g(Err(e))
//         }
//         me.slot.clone().on_full(move |slot| {
//             slot.try_consume().ok().unwrap();
//             me.schedule(g)
//         });
//     }
// }

impl Loop {
    pub fn new() -> io::Result<Loop> {
        Ok(Loop {
            io: try!(mio::EventLoop::new()),
            inner: Inner {
                done: HashMap::new(),
                next: 0,
            },
        })
    }

    pub fn await<F: Future>(&mut self, mut f: F)
                            -> FutureResult<F::Item, F::Error> {
        let mut ret = None;
        self._await(&mut || {
            ret = f.poll();
            ret.is_some()
        });
        Ok(try!(ret.unwrap()))
    }

    fn _await(&mut self, done: &mut FnMut() -> bool) {
        while !done() {
            self.io.run_once(&mut self.inner, None).unwrap();
        }
    }

    pub fn tcp_connect(&mut self, addr: &SocketAddr)
                       -> Box<IoFuture<TcpStream>> {
        let (p, c) = promise();
        let pair = mio::tcp::TcpStream::connect(addr).and_then(|tcp| {
            let token = self.inner.next;
            self.inner.next += 1;
            try!(self.io.register(&tcp,
                                  mio::Token(token),
                                  mio::EventSet::writable(),
                                  mio::PollOpt::edge() |
                                    mio::PollOpt::oneshot()));
            Ok((tcp, token))
        });
        match pair {
            Ok((tcp, token)) => {
                assert!(self.inner.done.insert(token, c).is_none());
                let io = self.io.channel();
                p.map(|()| {
                    TcpStream {
                        tcp: tcp,
                        io: io,
                    }
                }).boxed()
            }
            Err(e) => {
                c.fail(e);
                p.map(|()| panic!("should have failed")).boxed()
            }
        }
    }

    pub fn tcp_listen(&self, addr: &SocketAddr) -> io::Result<TcpListener> {
        let tcp = try!(mio::tcp::TcpListener::bind(addr));
        let io = self.io.channel();

        Ok(TcpListener {
            tcp: tcp,
            io: io,
        })
    }
}
//
// impl Inner {
//     pub fn await(&self, slot: &Slot<()>) {
//         let (reader, mut writer) = mio::unix::pipe().unwrap();
//         let mut events = mio::Events::new();
//         let mut done = false;
//         slot.on_full(move |_slot| {
//             use std::io::Write;
//             writer.write(&[1]).unwrap();
//         });
//         self.poll.register(&reader, mio::Token(0),
//                            mio::EventSet::readable(),
//                            mio::PollOpt::edge()).unwrap();
//         while !done {
//             self.poll.poll(&mut events, None).unwrap();
//             for event in events.iter() {
//                 if event.token() == mio::Token(0) {
//                     done = true;
//                     continue
//                 }
//                 let slot = unsafe { token2slot(event.token()) };
//                 let kind = event.kind();
//                 slot.on_empty(move |complete| {
//                     complete.try_produce(kind).ok().unwrap();
//                 });
//             }
//         }
//     }
// }
//
// unsafe fn token2slot(token: mio::Token) -> Arc<Slot<mio::EventSet>> {
//     mem::transmute(token.as_usize())
// }
//
// fn slot2token(c: Arc<Slot<mio::EventSet>>) -> mio::Token {
//     unsafe { mio::Token(mem::transmute::<_, usize>(c)) }
// }

impl mio::Handler for Inner {
    type Timeout = ();
    type Message = Message;

    fn ready(&mut self,
             _io: &mut mio::EventLoop<Self>,
             token: mio::Token,
             _events: mio::EventSet) {
        if let Some(token) = self.done.remove(&token.as_usize()) {
            token.finish(());
        }
        // println!("{:?}: {:?}", token, events);
    }

    fn notify(&mut self,
              _io: &mut mio::EventLoop<Self>,
              _msg: Message) {
        println!("msg");
    }
}
