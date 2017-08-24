use std::sync::Arc;
use std::thread;

use Async;
use stream::Stream;
use executor;
use task_impl::ThreadNotify;

#[derive(Debug)]
pub struct BlockingStream<S> {
    stream: executor::Spawn<S>,
}

impl<S> BlockingStream<S> {
    pub fn new(s: S) -> BlockingStream<S> where S: Stream {
        BlockingStream {
            stream: executor::spawn(s),
        }
    }

    pub fn get_ref(&self) -> &S {
        self.stream.get_ref()
    }

    pub fn get_mut(&mut self) -> &mut S {
        self.stream.get_mut()
    }

    pub fn into_inner(self) -> S {
        self.stream.into_inner()
    }
}

impl<S: Stream> Iterator for BlockingStream<S> {
    type Item = Result<S::Item, S::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        ThreadNotify::with_current(|notify| {
            loop {
                match self.stream.poll_stream_notify(notify, 0) {
                    Ok(Async::NotReady) => notify.park(),
                    Ok(Async::Ready(Some(e))) => return Some(Ok(e)),
                    Ok(Async::Ready(None)) => return None,
                    Err(e) => return Some(Err(e)),
                }
            }
        })
    }
}
