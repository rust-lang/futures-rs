//! A `Future` interface on top of libcurl
//!
//! This crate provides a futures-based interface to the libcurl HTTP library.
//! Building on top of the `curl` crate on crates.io, this allows using a
//! battle-tested C library for sending HTTP requests in an asynchronous
//! fashion.
//!
//! > **Note**: this crate currently only supports Unix, but Windows support is
//! >           coming soon!
//!
//! # Examples
//!
//! ```rust
//! extern crate curl;
//! extern crate futures;
//! extern crate futures_curl;
//! extern crate futures_mio;
//!
//! use std::io::{self, Write};
//!
//! use curl::easy::Easy;
//! use futures::Future;
//! use futures_mio::Loop;
//! use futures_curl::Session;
//!
//! fn main() {
//!     // Create an event loop that we'll run on, as well as an HTTP `Session`
//!     // which we'll be routing all requests through.
//!     let mut lp = Loop::new().unwrap();
//!     let session = Session::new(lp.pin());
//!
//!     // Prepare the HTTP request to be sent.
//!     let mut req = Easy::new();
//!     req.get(true).unwrap();
//!     req.url("https://www.rust-lang.org").unwrap();
//!     req.write_function(|data| {
//!         io::stdout().write_all(data).unwrap();
//!         Ok(data.len())
//!     }).unwrap();
//!
//!     // Once we've got our session, issue an HTTP request to download the
//!     // rust-lang home page
//!     let request = session.perform(req);
//!
//!     // Execute the request, and print the response code as well as the error
//!     // that happened (if any).
//!     let (mut req, err) = lp.run(request).unwrap();
//!     println!("{:?} {:?}", req.response_code(), err);
//! }
//! ```

#![deny(missing_docs)]

// TODO: windows support, should use a separate thread calling select() I guess
// TODO: handle level a bit better by turning the event loop every so often

#[macro_use]
extern crate log;
#[macro_use]
extern crate scoped_tls;
extern crate curl;
extern crate futures;
extern crate futures_io;
extern crate futures_mio;
extern crate libc;
extern crate mio;
extern crate slab;

use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::mem;
use std::rc::Rc;
use std::sync::{Arc, Weak};
use std::time::Duration;

use curl::Error;
use curl::easy::Easy;
use curl::multi::{Multi, EasyHandle, Socket, SocketEvents, Events};
use futures::{Future, Poll, oneshot, Oneshot, Complete};
use futures::task::{self, TaskHandle};
use futures_mio::{LoopPin, LoopData, Timeout, ReadinessStream};
use mio::unix::EventedFd;
use slab::Slab;

/// A shared cache for HTTP requests to pool data such as TCP connections
/// between.
///
/// All HTTP requests in this crate are performed through a `Session` type. A
/// `Session` can be cloned to acquire multiple references to the same session.
///
/// Sessions are created through the `Session::new` method, which returns a
/// future that will resolve to a session once it's been initialized.
#[derive(Clone)]
pub struct Session {
    data: Arc<LoopData<Rc<Data>>>,
}

struct Driver {
    data: Weak<LoopData<Rc<Data>>>,
}

struct Data {
    multi: Multi,
    state: RefCell<State>,
    pin: LoopPin,
}

struct State {
    // List of all pending HTTP requests, also what to do when they're done.
    complete: Slab<Entry, usize>,

    // Monotonically increasing token which is used to differentiate entries at
    // the same slab index over time. Primarily used in `PerformData`'s drop impl
    next_token: usize,

    // Active I/O for each known file descriptor curl is using. This is where we
    // get readiness notifications from.
    io: HashMap<Socket, SocketState>,

    // Timeout set by libcurl, and this is the handle to the actual timeout in
    // the event loop itself.
    timeout: Option<Timeout>,

    // If the "driver task" is waiting, then there'll be a handle here to wake
    // it up. Useful for when a new HTTP request is added or the like.
    waiting_task: Option<TaskHandle>,
}

struct Entry {
    complete: Complete<io::Result<(Easy, Option<Error>)>>,
    handle: EasyHandle,
    // What slab index this entry is at
    idx: usize,
    // Our current token, taken from `next_token` above, used to differentiate
    // two different entries at the same slab index.
    token: usize,
}

struct SocketState {
    want: Option<SocketEvents>,
    changed: bool,
    stream: ReadinessStream<MioSocket>,
}

scoped_thread_local!(static DATA: Data);

/// A future returned from the `Session::perform` method.
///
/// This future represents the execution of an entire HTTP request. This future
/// will resolve to the original `Easy` handle provided once the HTTP request is
/// complete so metadata about the request can be inspected.
pub struct Perform {
    state: PerformState,
    data: Arc<LoopData<Rc<Data>>>,
}

struct PerformData {
    data: Rc<Data>,
    idx: usize,     // slab index our entry is at
    token: usize,   // token we expect to see at that slab index
}

enum PerformState {
    Start(Easy),
    Scheduled(Oneshot<io::Result<(Easy, Option<Error>)>>,
              Option<LoopData<PerformData>>),
    Empty,
}

impl Session {
    /// Creates a new HTTP session object which will be bound to the given event
    /// loop.
    ///
    /// When using libcurl it will provide us with file descriptors to listen
    /// for events on, so we'll need raw access to an actual event loop in order
    /// to hook up all the pieces together. The event loop will also be the I/O
    /// home for this HTTP session. All HTTP I/O will occur on the event loop
    /// thread.
    ///
    /// This function returns a future which will resolve to a `Session` once
    /// it's been initialized.
    pub fn new(pin: LoopPin) -> Session {
        // The core part of a `Session` is its `LoopData`, so we create that
        // here. The data itself is just a `Multi`, and to kick it off we
        // configure the timer/socket functions will receive notifications on
        // new timeouts and new events to listen for on sockets.
        let mut m = Multi::new();

        m.timer_function(move |dur| {
            DATA.with(|d| d.schedule_timeout(dur))
        }).unwrap();

        m.socket_function(move |socket, events, token| {
            DATA.with(|d| d.schedule_socket(socket, events, token))
        }).unwrap();

        let pin2 = pin.clone();
        let data = pin.add_loop_data(Rc::new(Data {
            multi: m,
            pin: pin2,
            state: RefCell::new(State {
                next_token: 0,
                complete: Slab::new(128),
                timeout: None,
                waiting_task: None,
                io: HashMap::new(),
            }),
        }));

        // Once we've got the data stick the `LoopData` reference in an `Arc` so
        // we can share it amongst futures.
        let data = Arc::new(data);
        Driver { data: Arc::downgrade(&data) }.forget();
        Session { data: data }
    }

    /// Execute and HTTP request asynchronously, returning a future representing
    /// the request's completion.
    ///
    /// This method will consume the provided `Easy` handle, which should be
    /// configured appropriately to send off an HTTP request. The returned
    /// future will resolve back to the handle once the request is performed,
    /// along with any error that happened along the way.
    ///
    /// The `Item` of the returned future is `(Easy, Option<Error>)` so you can
    /// always get the `Easy` handle back, and the `Error` part of the future is
    /// `io::Error` which represents errors communicating with the event loop or
    /// otherwise fatal errors for the `Easy` provided.
    ///
    /// Note that if the `Perform` future is dropped it will cancel the
    /// outstanding HTTP request, cleaning up all resources associated with it.
    ///
    /// Note that all callbacks associated with the `Easy` handle, for example
    /// the read and write functions, will get executed on the event loop. As a
    /// result you may want to close over `LoopData` in them if you'd like to
    /// collect the results.
    pub fn perform(&self, handle: Easy) -> Perform {
        Perform {
            data: self.data.clone(),
            state: PerformState::Start(handle),
        }
    }
}

impl Data {
    /// Function called whenever a new timeout is requested from libcurl.
    ///
    /// An argument of `None` indicates the current timeout can be cleared, and
    /// otherwise this indicates a new timeout to set for informing libcurl that
    /// a timeout has happened.
    fn schedule_timeout(&self, dur: Option<Duration>) -> bool {
        // First up, always clear the existing timeout
        let mut state = self.state.borrow_mut();
        state.timeout = None;

        // If a timeout was requested, then we configure one. Note that we know
        // for sure that we're executing on the event loop because `Data` is
        // owned by the event loop thread. As a result the returned future from
        // `LoopHandle::timeout` should be immediately resolve-able, so we do so
        // here to pull out the actual timeout future.
        if let Some(dur) = dur {
            debug!("scheduling a new timeout in {:?}", dur);
            let mut timeout = self.pin.handle().clone().timeout(dur);
            let mut timeout = match timeout.poll() {
                Poll::Ok(timeout) => timeout,
                _ => panic!("event loop should finish poll immediately"),
            };
            drop(state);
            let res = timeout.poll();
            state = self.state.borrow_mut();
            match res {
                Poll::NotReady => state.timeout = Some(timeout),
                Poll::Ok(()) => panic!("timeout done immediately?"),
                Poll::Err(e) => panic!("timeout poll error: {}", e),
            }
        }

        true
    }

    /// Function called whenever libcurl requests events to be listened for on a
    /// socket.
    ///
    /// This function is informed of the raw socket file descriptor, `socket`,
    /// the events that we're interested in, `events`, as well as a user-defined
    /// token, `token`. It's up to us to ensure that we're waiting appropriately
    /// for these events to happen, and then we'll later inform libcurl when
    /// they actually happen.
    fn schedule_socket(&self,
                       socket: Socket,
                       events: SocketEvents,
                       _token: usize) {
        let mut state = self.state.borrow_mut();

        // First up, if libcurl wants us to forget about this socket, we do so!
        if events.remove() {
            debug!("remove socket: {}", socket);
            state.io.remove(&socket).unwrap();
            return
        }

        // If this is the first time we've seen the socket then we register a
        // new source with the event loop. Currently that's done through
        // `ReadinessStream` which handles registration and deregistration of
        // interest on the event loop itself.
        //
        // Like above with timeouts, the future returned from `ReadinessStream`
        // should be immediately resolve-able because we're guaranteed to be on
        // the event loop.
        if !state.io.contains_key(&socket) {
            debug!("schedule new socket {}", socket);
            let source = MioSocket { inner: socket };
            let mut ready = ReadinessStream::new(self.pin.handle().clone(),
                                                 source);
            let stream = match ready.poll() {
                Poll::Ok(stream) => stream,
                _ => panic!("event loop should finish poll immediately"),
            };
            state.io.insert(socket, SocketState {
                stream: stream,
                want: None,
                changed: false,
            });
        } else {
            debug!("socket already registered: {}", socket);
        }

        let state = state.io.get_mut(&socket).unwrap();
        state.stream.need_read();
        state.stream.need_write();
        state.want = Some(events);
        state.changed = true;
    }

    /// Executes a new request, returning half of a oneshot that'll get filled
    /// in when the future is done.
    ///
    /// This oneshot can migrate to other threads safely and we'll ensure that
    /// it gets filled in appropriately on the event loop thread.
    fn execute(me: &Rc<Data>, req: Easy)
               -> (Oneshot<io::Result<(Easy, Option<Error>)>>,
                   Option<LoopData<PerformData>>) {
        // This is pretty straightforward, the intention being to call the
        // `Multi::add` function which adds the handle to the libcurl multi
        // handle.
        //
        // The tricky part here is that if the driver task is blocking then we
        // have to be sure to wake it up or otherwise it may never otherwise see
        // the new request.
        debug!("executing a new request");
        let (tx, rx) = oneshot();
        let handle = match DATA.set(me, || me.multi.add(req)) {
            Ok(handle) => handle,
            Err(e) => {
                tx.complete(Err(e.into()));
                return (rx, None)
            }
        };
        debug!("handle added");
        let mut state = me.state.borrow_mut();
        if state.complete.vacant_entry().is_none() {
            let amt = state.complete.count();
            state.complete.grow(amt);
        }
        let idx;
        let token = state.next_token;
        state.next_token += 1;
        {
            let entry = state.complete.vacant_entry().unwrap();
            idx = entry.index();
            entry.insert(Entry {
                complete: tx,
                handle: handle,
                idx: idx,
                token: token,
            });
        }
        let task = state.waiting_task.take();
        drop(state);
        if let Some(t) = task {
            debug!("notifying task");
            t.unpark();
        }
        let data = me.pin.add_loop_data(PerformData {
            data: me.clone(),
            idx: idx,
            token: token,
        });
        (rx, Some(data))
    }

    /// Polls the internal state of this `Multi` handle, attempting to move the
    /// world forward.
    fn poll(&self) -> Poll<(), ()> {
        debug!("-------------------------- driver poll start");

        // First up, process socket events. We take a look at all our registered
        // streams to see which ones of them became ready.
        //
        // TODO: should measure the perf here, just a few atomics but worth
        //       double-checking it's not too expensive
        let mut events = Vec::new();
        for (socket, state) in self.state.borrow_mut().io.iter_mut() {
            trace!("curl[{}] checking socket", socket);

            let mut e = Events::new();
            let mut set = false;
            if let Poll::Ok(()) = state.stream.poll_read() {
                debug!("\treadable");
                e.input(true);
                set = true;
            }
            if let Poll::Ok(()) = state.stream.poll_write() {
                debug!("\twritable");
                e.output(true);
                set = true;
            }

            // If something was set then we'll tell libcurl about it in a
            // moment.
            if set {
                events.push((*socket, e));
            }
        }

        DATA.set(self, || {
            // After we've figured out what events are available, we then inform
            // libcurl of all these events.
            //
            // libcurl wants "level" behavior instead of edge which we have
            // by default, so we have to do a bit of translation here to work
            // around that. Our `poll_read` and `poll_write` calls above will
            // get *set* on an edge basis, and then we have to decide when to
            // turn them off. Normally EAGAIN does this but we aren't in a
            // position to listen for that unfortunately.
            //
            // To handle this we do a few things:
            //
            // 1. If libcurl calls our socket callback for this socket as part
            //    of the call to `action` below, then it'll flag the `changed`
            //    field and we assume that we don't need to schedule another
            //    call to `action`. The socket callback calls `need_read` and
            //    `need_write` as well, clearing their readiness until epoll
            //    tells us again.
            //
            // 2. If libcurl *didn't* update us through the socket callback,
            //    then it means that libcurl has the same "want" as before. We
            //    don't actually know if the socket itself is readable or
            //    writable, unfortunately, and we don't really have any way of
            //    finding out unless we test it ourselves. For that reason we
            //    call `libc::poll` manually.
            //
            // Once the call to `poll` returns we look at the `revents` field
            // the kernel should have filled in. If something about that and our
            // `want` set disagrees, we flag the readiness stream with `need_*`
            // and then go to the next event.
            //
            // If, however, `poll` returns that the socket is still in the
            // appropriate state to hand to libcurl (e.g. the level
            // notification) then we enqueue a poll of the socket for another
            // turn of the event loop through the `poll_on` method.
            for &(socket, ref events) in events.iter() {
                self.state.borrow_mut().io.get_mut(&socket).unwrap().changed = false;
                self.multi.action(socket, events).expect("action error");

                let mut state = self.state.borrow_mut();
                let state = match state.io.get_mut(&socket) {
                    Some(state) => state,
                    None => continue,
                };
                if state.changed {
                    continue
                }
                let want = match state.want {
                    Some(ref want) => want,
                    None => continue,
                };

                let mut fd = libc::pollfd {
                    fd: socket,
                    events: 0,
                    revents: 0,
                };
                if want.input() {
                    fd.events |= libc::POLLIN;
                }
                if want.output() {
                    fd.events |= libc::POLLOUT;
                }
                unsafe {
                    libc::poll(&mut fd, 1, 0);
                }
                if want.input() && (fd.revents & libc::POLLIN) == 0 {
                    state.stream.need_read();
                    continue
                }
                if want.output() && (fd.revents & libc::POLLOUT) == 0 {
                    state.stream.need_write();
                    continue
                }
                task::poll_on(self.pin.executor());
            }

            // Process a timeout, if one ocurred.
            //
            // If our `timeout` field is set then we check that future, and if
            // it fires then we destroy it and inform libcurl that a timeout has
            // happened.
            let mut timeout = false;
            if let Some(ref mut t) = self.state.borrow_mut().timeout {
                if let Poll::Ok(()) = t.poll() {
                    timeout = true;
                }
            }
            if timeout {
                debug!("timeout fired");
                self.state.borrow_mut().timeout = None;
                self.multi.timeout().expect("timeout error");
            }

            // After all that's done, we check to see if any transfers have
            // completed.
            //
            // This function is where we'll actually complete the associated
            // futures.
            self.multi.messages(|m| {
                debug!("a request is done!");
                let mut state = self.state.borrow_mut();
                let transfer_err = m.result().unwrap();
                let idx = state.complete.iter()
                                        .find(|e| m.is_for(&e.handle))
                                        .expect("complete but handle not here")
                                        .idx;
                let entry = state.complete.remove(idx).unwrap();
                drop(state);

                // If `remove_err` fails then that's super fatal, so that'll end
                // up in the `Error` of the `Perform` future. If, however, the
                // transfer just failed, then that's communicated through
                // `transfer_err`, so we just put that next to the handle if we
                // get it out successfully.
                let remove_err = self.multi.remove(entry.handle);
                let res = remove_err.map(|e| (e, transfer_err.err()))
                                    .map_err(|e| e.into());
                entry.complete.complete(res);
            });
        });

        if self.state.borrow().waiting_task.is_none() {
            self.state.borrow_mut().waiting_task = Some(task::park());
        }

        Poll::NotReady
    }

    fn cancel(&self, idx: usize, token: usize) {
        let mut state = self.state.borrow_mut();
        match state.complete.get(idx) {
            Some(e) if e.token == token => {}
            _ => return,
        }
        let entry = state.complete.remove(idx).unwrap();
        drop(state);
        DATA.set(self, move || {
            drop(self.multi.remove(entry.handle));
        });
    }
}

// When a session is being destroyed, be sure to wake up the corresponding task
// if any is sleeping to ensure that it gets deallocated as well.
impl Drop for State {
    fn drop(&mut self) {
        if let Some(task) = self.waiting_task.take() {
            task.unpark();
        }
    }
}

impl Future for Perform {
    type Item = (Easy, Option<Error>);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, io::Error> {
        loop {
            match mem::replace(&mut self.state, PerformState::Empty) {
                PerformState::Start(easy) => {
                    // Home back to the event loop to figure out if we're done yet.
                    let data = match self.data.get() {
                        Some(data) => data,
                        None => {
                            task::poll_on(self.data.executor());
                            self.state = PerformState::Start(easy);
                            return Poll::NotReady
                        }
                    };

                    // Once we're on the event loop execute the request and store
                    // off the saved future.
                    let (handle, dtor) = Data::execute(data, easy);
                    self.state = PerformState::Scheduled(handle, dtor);
                }
                PerformState::Scheduled(mut s, dtor) => {
                    debug!("polling a scheduled request");
                    match s.poll() {
                        Poll::Ok(Ok(e)) => return Poll::Ok(e),
                        Poll::Ok(Err(e)) => return Poll::Err(e),
                        Poll::Err(_) => panic!("complete canceled?"),
                        Poll::NotReady => {
                            self.state = PerformState::Scheduled(s, dtor);
                            return Poll::NotReady
                        }
                    }
                }
                PerformState::Empty => panic!("poll on empty Perform"),
            }
        }
    }
}

impl Drop for PerformData {
    fn drop(&mut self) {
        self.data.cancel(self.idx, self.token);
    }
}

impl Future for Driver {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        match self.data.upgrade() {
            Some(arc) => {
                match arc.get() {
                    Some(data) => data.poll(),
                    None => {
                        task::poll_on(arc.executor());
                        Poll::NotReady
                    }
                }
            }
            None => Poll::Ok(()),
        }
    }
}

struct MioSocket {
    inner: curl::multi::Socket,
}

impl mio::Evented for MioSocket {
    fn register(&self,
                poll: &mio::Poll,
                token: mio::Token,
                interest: mio::EventSet,
                opts: mio::PollOpt) -> io::Result<()> {
        EventedFd(&self.inner).register(poll, token, interest, opts)
    }

    fn reregister(&self,
                  poll: &mio::Poll,
                  token: mio::Token,
                  interest: mio::EventSet,
                  opts: mio::PollOpt) -> io::Result<()> {
        EventedFd(&self.inner).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> io::Result<()> {
        EventedFd(&self.inner).deregister(poll)
    }
}
