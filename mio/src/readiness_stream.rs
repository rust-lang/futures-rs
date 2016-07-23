#![allow(missing_docs)] // TODO: document this module

use std::io;
use std::sync::Arc;

use futures::io::Ready;
use futures::stream::Stream;
use futures::{Future, Task, Poll};

use IoFuture;
use event_loop::{LoopHandle, Source};
use readiness_stream::drop_source::DropSource;


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
    state: State,
    io_token: usize,
    read_token: usize,
    write_token: usize,
    loop_handle: LoopHandle,
    _drop_source: Arc<DropSource>,
}

impl ReadinessStream {
    pub fn new(loop_handle: LoopHandle, source: Source)
               -> Box<IoFuture<ReadinessStream>> {
        loop_handle.add_source(source).map(|token| {
            let drop_source = Arc::new(DropSource::new(token, loop_handle.clone()));
            ReadinessStream {
                state: State::NeverPolled,
                io_token: token,
                read_token: 2 * token,
                write_token: 2 * token + 1,
                loop_handle: loop_handle,
                _drop_source: drop_source,
            }
        }).boxed()
    }
}

impl Stream for ReadinessStream {
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        if !self.state.ready_on_poll() {
            Poll::NotReady
        } else if task.may_contain(self.read_token) {
            self.state = State::Polled;
            if task.may_contain(self.write_token) {
                Poll::Ok(Some(Ready::ReadWrite))
            } else {
                Poll::Ok(Some(Ready::Read))
            }
        } else if task.may_contain(self.write_token) {
            self.state = State::Polled;
            Poll::Ok(Some(Ready::Write))
        } else {
            Poll::NotReady
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        // TODO: need to update the wake callback
        if self.state != State::Scheduled {
            self.state = State::Scheduled;
            self.loop_handle.schedule(self.io_token, task)
        }
    }
}

impl Drop for ReadinessStream {
    fn drop(&mut self) {
        self.loop_handle.deschedule(self.io_token)
    }
}
