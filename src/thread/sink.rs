use std::sync::Arc;
use std::thread;

use AsyncSink;
use sink::Sink;
use executor;
use task_impl::ThreadNotify;

#[derive(Debug)]
pub struct BlockingSink<S> {
    sink: executor::Spawn<S>,
}

impl<S> BlockingSink<S> {
    pub fn new(s: S) -> BlockingSink<S> where S: Sink {
        BlockingSink {
            sink: executor::spawn(s),
        }
    }

    pub fn get_ref(&self) -> &S {
        self.sink.get_ref()
    }

    pub fn get_mut(&mut self) -> &mut S {
        self.sink.get_mut()
    }

    pub fn into_inner(self) -> S {
        self.sink.into_inner()
    }

    pub fn send(&mut self, mut value: S::SinkItem) -> Result<(), S::SinkError>
        where S: Sink
    {
        ThreadNotify::with_current(|notify| {
            loop {
                value = match self.sink.start_send_notify(value, notify, 0)? {
                    AsyncSink::NotReady(v) => v,
                    AsyncSink::Ready => return Ok(()),
                };
                notify.park();
            }
        })
    }

    pub fn flush(&mut self) -> Result<(), S::SinkError>
        where S: Sink
    {
        ThreadNotify::with_current(|notify| {
            loop {
                if self.sink.poll_flush_notify(notify, 0)?.is_ready() {
                    return Ok(())
                }
                notify.park();
            }
        })
    }

    pub fn close(&mut self) -> Result<(), S::SinkError>
        where S: Sink
    {
        ThreadNotify::with_current(|notify| {
            loop {
                if self.sink.close_notify(notify, 0)?.is_ready() {
                    return Ok(())
                }
                notify.park();
            }
        })
    }
}
