use std::thread;

use {Poll, ThreadNotify};
use stream::Stream;
use task::Task;

/// A stream combinator which converts an asynchronous stream to a **blocking
/// iterator**.
///
/// Created by the `Stream::await` method, this function transforms any stream
/// into a standard iterator. This is implemented by blocking the current thread
/// while items on the underlying stream aren't ready yet.
pub struct Wait<S> {
    task: Task,
    stream: S,
}

pub fn new<S: Stream>(s: S) -> Wait<S> {
    Wait {
        task: Task::new_notify(ThreadNotify(thread::current())),
        stream: s,
    }
}

impl<S: Stream> Iterator for Wait<S> {
    type Item = Result<S::Item, S::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let stream = &mut self.stream;
        loop {
            match self.task.enter(|| stream.poll()) {
                Poll::Ok(Some(e)) => return Some(Ok(e)),
                Poll::Ok(None) => return None,
                Poll::Err(e) => return Some(Err(e)),
                Poll::NotReady => thread::park(),
            }
        }
    }
}
