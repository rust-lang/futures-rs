#![allow(missing_docs)] // TODO: document this module

use super::event_loop::{Direction, LoopHandle};

use self::drop_source::DropSource;

use mio;

use std::io;
use std::sync::Arc;

use futures::{Future, Tokens, Wake, Poll};
use futures::stream::Stream;

// TODO: figure out a nicer way to factor this
mod drop_source {
    use event_loop::LoopHandle;

    pub struct DropSource {
        token: usize,
        loop_handle: LoopHandle,
    }

    impl DropSource {
        pub fn new(token: usize, loop_handle: LoopHandle) -> DropSource {
            DropSource {
                token: token,
                loop_handle: loop_handle,
            }
        }
    }

    // Safe because no public access exposed to LoopHandle; only used in drop
    unsafe impl Sync for DropSource {}

    impl Drop for DropSource {
        fn drop(&mut self) {
            self.loop_handle.drop_source(self.token)
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum State {
    NeverPolled,
    Polled,
    Scheduled,
}

impl State {
    fn ready_on_poll(&self) -> bool {
        match *self {
            State::NeverPolled => true,
            State::Scheduled => true,
            State::Polled => false,
        }
    }
}

pub struct ReadinessStream {
    dir: Direction,
    state: State,
    token: usize,
    token_to_test: usize,
    loop_handle: LoopHandle,
    _drop_source: Arc<DropSource>,
}

impl ReadinessStream {
    pub fn dir(&self) -> Direction {
        self.dir
    }
}

impl Stream for ReadinessStream {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self, tokens: &Tokens) -> Poll<Option<()>, io::Error> {
        if self.state.ready_on_poll() && tokens.may_contain(self.token_to_test) {
            self.state = State::Polled;
            Poll::Ok(Some(()))
        } else {
            Poll::NotReady
        }
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        // TODO: need to update the wake callback
        if self.state != State::Scheduled {
            self.state = State::Scheduled;
            self.loop_handle.schedule(self.token, self.dir, wake)
        }
    }
}

impl Drop for ReadinessStream {
    fn drop(&mut self) {
        self.loop_handle.deschedule(self.token, self.dir)
    }
}

pub struct ReadinessPair<T> {
    pub source: Arc<T>,
    pub ready_read: ReadinessStream,
    pub ready_write: ReadinessStream,
}

impl<E> ReadinessPair<E> where E: Send + Sync + mio::Evented + 'static {
    pub fn new(loop_handle: LoopHandle, event: E)
               -> Box<Future<Item=ReadinessPair<E>, Error=io::Error>> {
        let event = Arc::new(event);
        loop_handle.add_source(event.clone()).and_then(|token| {
            let drop_source = Arc::new(DropSource::new(token, loop_handle.clone()));
            Ok(ReadinessPair {
                source: event,
                ready_read: ReadinessStream {
                    dir: Direction::Read,
                    state: State::NeverPolled,
                    token: token,
                    token_to_test: 2 * token,
                    loop_handle: loop_handle.clone(),
                    _drop_source: drop_source.clone(),
                },
                ready_write: ReadinessStream {
                    dir: Direction::Write,
                    state: State::NeverPolled,
                    token: token,
                    token_to_test: 2 * token + 1,
                    loop_handle: loop_handle,
                    _drop_source: drop_source,
                },
            })
        }).boxed()
    }
}
