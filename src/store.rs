use std::marker;
use std::any::Any;

use {Task, TaskData, Poll, Future};

/// A combinator which will store some data into task-local storage.
///
/// This combinator is created by the `futures::store` method.
pub struct Store<T: Send + 'static, E> {
    item: Option<T>,
    _marker: marker::PhantomData<fn() -> E>,
}

/// A combinator to store some data into task-local storage.
pub fn store<T, E>(t: T) -> Store<T, E>
    where T: Any + Send + 'static,
          E: Send + 'static,
{
    Store { item: Some(t), _marker: marker::PhantomData }
}

impl<T, E> Future for Store<T, E>
    where T: Any + Send + 'static,
          E: Send + 'static,
{
    type Item = TaskData<T>;
    type Error = E;

    fn poll(&mut self, task: &mut Task) -> Poll<TaskData<T>, E> {
        let item = self.item.take().expect("cannot poll Store twice");
        Poll::Ok(task.insert(item))
    }

    fn schedule(&mut self, task: &mut Task) {
        task.notify()
    }
}
