use std::thread;
use std::sync::Arc;

use {Poll, ThreadNotify};
use stream::Stream;
use task::{Task, Unpark};

/// A stream combinator which converts an asynchronous stream to a **blocking
/// iterator**.
///
/// Created by the `Stream::await` method, this function transforms any stream
/// into a standard iterator. This is implemented by blocking the current thread
/// while items on the underlying stream aren't ready yet.
pub struct Wait<S> {
    unpark: Arc<Unpark>,
    task: Task,
    stream: S,
}

pub fn new<S: Stream>(s: S) -> Wait<S> {
    Wait {
        unpark: Arc::new(ThreadNotify(thread::current())),
        task: Task::new(),
        stream: s,
    }
}

impl<S: Stream> Iterator for Wait<S> {
    type Item = Result<S::Item, S::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let stream = &mut self.stream;
        loop {
            match self.task.enter(&self.unpark, || stream.poll()) {
                Poll::Ok(Some(e)) => return Some(Ok(e)),
                Poll::Ok(None) => return None,
                Poll::Err(e) => return Some(Err(e)),
                Poll::NotReady => thread::park(),
            }
        }
    }
}
