#![allow(missing_docs)] // TODO: document this module

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::io::Ready;
use futures::stream::Stream;
use futures::{store_notify, Future, Task, Poll, TaskNotifyData};

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

pub struct ReadinessStream {
    io_token: usize,
    data: TaskNotifyData<AtomicUsize>,
    loop_handle: LoopHandle,
    _drop_source: Arc<DropSource>,
}

impl ReadinessStream {
    pub fn new(loop_handle: LoopHandle, source: Source)
               -> Box<IoFuture<ReadinessStream>> {
        loop_handle.add_source(source).and_then(|token| {
            store_notify(AtomicUsize::new(0)).map(move |data| (token, data))
        }).map(|(token, data)| {
            let drop_source = Arc::new(DropSource::new(token, loop_handle.clone()));
            ReadinessStream {
                io_token: token,
                data: data,
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
        let data = task.handle().get(&self.data).swap(0, Ordering::Relaxed);
        match data {
            0 => Poll::NotReady,
            1 => Poll::Ok(Some(Ready::Read)),
            2 => Poll::Ok(Some(Ready::Write)),
            3 => Poll::Ok(Some(Ready::ReadWrite)),
            _ => panic!(),
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        self.loop_handle.schedule(self.io_token, task, self.data)
    }
}

impl Drop for ReadinessStream {
    fn drop(&mut self) {
        self.loop_handle.deschedule(self.io_token)
    }
}
