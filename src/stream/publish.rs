use std::clone::Clone;
use std::mem;
use std::sync::Arc;
use std::sync::Mutex;
use std::vec::Vec;

use stream::Stream;
use Poll;
use task::{self, Task};

/// A stream combinator which multicasts a stream.
///
/// This structure is produced by the `Stream::publish` method.
pub struct Publish<S>
    where S: Stream,
          S::Item: Clone,
          S::Error: Clone,
{
    task: Option<Task>,
    inner: Arc<Mutex<PublishInner<S>>>
}

pub fn new<S>(stream: S) -> Publish<S>
    where S: Stream,
          S::Item: Clone,
          S::Error: Clone,
{
    Publish {
        task: None,
        inner: Arc::new(Mutex::new(PublishInner::new(stream)))
    }
}

impl<S> Stream for Publish<S>
    where S: Stream,
          S::Item: Clone,
          S::Error: Clone,
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        let mut inner = self.inner.lock().unwrap();
        if self.task.is_none() {
            let task = task::park();
            inner.add_task(task.clone());
            self.task = Some(task)
        }
        inner.poll()
    }
}

impl<S> Clone for Publish<S>
    where S: Stream,
          S::Item: Clone,
          S::Error: Clone,
{
    fn clone(&self) -> Publish<S> {
        Publish {
            task: None,
            inner: self.inner.clone()
        }
    }
}

struct PublishInner<S>
    where S: Stream,
          S::Item: Clone,
          S::Error: Clone,
{
    stream: S,
    items: Vec<Task>,
    remaining: usize,
    item: Poll<Option<S::Item>, S::Error>,
}

impl<S> PublishInner<S>
    where S: Stream,
          S::Item: Clone,
          S::Error: Clone,
{
    fn new(stream: S) -> PublishInner<S> {
        PublishInner {
            stream: stream,
            items: Vec::new(),
            remaining: 0,
            item: Poll::NotReady
        }
    }

    fn add_task(&mut self, task: Task) {
        self.items.push(task);
        if self.item.is_ready() {
            self.remaining += 1
        }
    }

    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        if self.item.is_ready() {
            self.remaining -= 1;
        } else {
            self.item = self.stream.poll();
            if self.item.is_ready() {
                for task in &self.items {
                    if !task.is_current() {
                        task.unpark()
                    }
                }
                self.remaining = self.items.len() - 1;
            }
        }
        if self.remaining == 0 {
            mem::replace(&mut self.item, Poll::NotReady)
        } else {
            self.item.clone()
        }
    }
}

