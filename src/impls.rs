use std::mem;

use {Future, empty, Poll, Task};

impl<T, E> Future for Box<Future<Item=T, Error=E>>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self, task: &mut Task) -> Poll<Self::Item, Self::Error> {
        (**self).poll(task)
    }

    fn schedule(&mut self, task: &mut Task) {
        (**self).schedule(task)
    }

    fn tailcall(&mut self)
                -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        if let Some(f) = (**self).tailcall() {
            return Some(f)
        }
        Some(mem::replace(self, Box::new(empty())))
    }
}

impl<F: Future> Future for Box<F> {
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Self::Item, Self::Error> {
        (**self).poll(task)
    }

    fn schedule(&mut self, task: &mut Task) {
        (**self).schedule(task)
    }

    fn tailcall(&mut self)
                -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        (**self).tailcall()
    }
}
