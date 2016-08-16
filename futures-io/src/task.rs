use std::cell::RefCell;
use std::io;

use futures::{Future, Task, TaskData, Poll, store, Store};
use futures::stream::Stream;

use {WriteTask, ReadTask, Ready};

/// Abstraction that allows inserting an I/O object into task-local storage,
/// returning a handle that can be split.
///
/// A `TaskIo<T>` handle implements the `ReadTask` and `WriteTask` and will only
/// work with the same task that the associated object was inserted into. The
/// handle may then be optionally `split` into the read/write halves so they can
/// be worked with independently.
///
/// Note that it is important that the future returned from `TaskIo::new`, when
/// polled, will pin the yielded `TaskIo<T>` object to that specific task. Any
/// attempt to read or write the object on other tasks will result in a panic.
pub struct TaskIo<T> {
    handle: TaskData<RefCell<Option<State<T>>>>,
}

/// A future returned from `TaskIo::new` which resolves to a new instance of
/// `TaskIo<T>` when resolved.
pub struct TaskIoNew<T> {
    inner: Store<RefCell<Option<State<T>>>, io::Error>,
}

/// The readable half of a `TaskIo<T>` instance returned from `TaskIo::split`.
///
/// This handle implements the `ReadTask` trait and can be used to split up an
/// I/O object into two distinct halves.
pub struct TaskIoRead<T> {
    handle: TaskData<RefCell<Option<State<T>>>>,
}

/// The writable half of a `TaskIo<T>` instance returned from `TaskIo::split`.
///
/// This handle implements the `WriteTask` trait and can be used to split up an
/// I/O object into two distinct halves.
pub struct TaskIoWrite<T> {
    handle: TaskData<RefCell<Option<State<T>>>>,
}

struct State<T> {
    object: T,
    ready: Option<Ready>,
}

impl<T: 'static> TaskIo<T> {
    /// Returns a new future which represents the insertion of the I/O object
    /// `T` into task local storage, returning a `TaskIo<T>` handle to it.
    ///
    /// The returned future will never resolve to an error.
    pub fn new(t: T) -> TaskIoNew<T> {
        let state = State {
            object: t,
            ready: None,
        };
        TaskIoNew {
            inner: store(RefCell::new(Some(state))),
        }
    }
}

impl<T: 'static> Future for TaskIoNew<T> {
    type Item = TaskIo<T>;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<TaskIo<T>, io::Error> {
        self.inner.poll(task).map(|data| TaskIo { handle: data })
    }
}

impl<T> TaskIo<T>
    where T: ReadTask + WriteTask,
{
    /// For an I/O object which is both readable and writable, this method can
    /// be used to split the handle into two independently owned halves.
    ///
    /// The returned pair implements the `ReadTask` and `WriteTask` traits,
    /// respectively, and can be used to pass around the object to different
    /// combinators if necessary.
    pub fn split(self) -> (TaskIoRead<T>, TaskIoWrite<T>) {
        (TaskIoRead { handle: self.handle.clone() },
         TaskIoWrite { handle: self.handle })
    }
}

struct TaskIoTake<'a, 'b, T: 'static> {
    t: Option<State<T>>,
    task: &'a mut Task,
    handle: &'b TaskData<RefCell<Option<State<T>>>>,
}

impl<'a, 'b, T: 'static> TaskIoTake<'a, 'b, T> {
    fn new(task: &'a mut Task, handle: &'b TaskData<RefCell<Option<State<T>>>>)
           -> TaskIoTake<'a, 'b, T> {
        let t = task.get(handle).borrow_mut().take().unwrap();
        TaskIoTake {
            t: Some(t),
            task: task,
            handle: handle,
        }
    }

    fn state(&mut self) -> &mut State<T> {
        self.t.as_mut().unwrap()
    }
}

impl<'a, 'b, T> TaskIoTake<'a, 'b, T>
    where T: Stream<Item=Ready, Error=io::Error>
{
    fn poll(&mut self, ready: Ready) -> Poll<Option<Ready>, io::Error> {
        let state = self.t.as_mut().unwrap();
        match (ready, state.ready.take()) {
            (_, None) => {}
            (other, Some(Ready::ReadWrite)) |
            (Ready::ReadWrite, Some(other)) => return Poll::Ok(Some(other)),
            (Ready::Read, Some(Ready::Read)) => return Poll::Ok(Some(Ready::Read)),
            (Ready::Write, Some(Ready::Write)) => return Poll::Ok(Some(Ready::Write)),
            (Ready::Write, cur @ Some(Ready::Read)) |
            (Ready::Read, cur @ Some(Ready::Write)) => {
                state.ready = cur;
            }
        }
        let found = match try_poll!(state.object.poll(self.task)) {
            Ok(None) => return Poll::Ok(None),
            Err(e) => return Poll::Err(e),
            Ok(Some(r)) => r,
        };

        let current = state.ready.take().unwrap_or(found) | found;
        match (ready, current) {
            (Ready::ReadWrite, other) => Poll::Ok(Some(other)),
            (Ready::Read, Ready::Read) => Poll::Ok(Some(Ready::Read)),
            (Ready::Write, Ready::Write) => Poll::Ok(Some(Ready::Write)),
            (Ready::Read, Ready::ReadWrite) => {
                state.ready = Some(Ready::Write);
                Poll::Ok(Some(Ready::Read))
            }
            (Ready::Write, Ready::ReadWrite) => {
                state.ready = Some(Ready::Read);
                Poll::Ok(Some(Ready::Write))
            }
            (Ready::Read, Ready::Write) => {
                state.ready = Some(Ready::Write);
                Poll::NotReady
            }
            (Ready::Write, Ready::Read) => {
                state.ready = Some(Ready::Read);
                Poll::NotReady
            }
        }
    }
}

impl<'a, 'b, T: 'static> Drop for TaskIoTake<'a, 'b, T> {
    fn drop(&mut self) {
        let t = self.t.take();
        *self.task.get(&self.handle).borrow_mut() = t;
    }
}

impl<T> Stream for TaskIo<T>
    where T: Stream<Item=Ready, Error=io::Error>,
{
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        TaskIoTake::new(task, &self.handle).poll(Ready::ReadWrite)
    }
}

impl<T> ReadTask for TaskIo<T>
    where T: io::Read + Stream<Item=Ready, Error=io::Error>,
{
    fn read(&mut self, task: &mut Task, buf: &mut [u8]) -> io::Result<usize> {
        let mut io = TaskIoTake::new(task, &self.handle);
        io.state().object.read(buf)
    }

    fn read_to_end(&mut self,
                   task: &mut Task,
                   buf: &mut Vec<u8>) -> io::Result<usize> {
        let mut io = TaskIoTake::new(task, &self.handle);
        io.state().object.read_to_end(buf)
    }
}

impl<T> WriteTask for TaskIo<T>
    where T: io::Write + Stream<Item=Ready, Error=io::Error>,
{
    fn write(&mut self, task: &mut Task, buf: &[u8]) -> io::Result<usize> {
        let mut io = TaskIoTake::new(task, &self.handle);
        io.state().object.write(buf)
    }

    fn flush(&mut self, task: &mut Task) -> io::Result<()> {
        let mut io = TaskIoTake::new(task, &self.handle);
        io.state().object.flush()
    }
}

impl<T> Stream for TaskIoRead<T>
    where T: Stream<Item=Ready, Error=io::Error>,
{
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        TaskIoTake::new(task, &self.handle).poll(Ready::Read)
    }
}

impl<T> ReadTask for TaskIoRead<T>
    where T: io::Read + Stream<Item=Ready, Error=io::Error>,
{
    fn read(&mut self, task: &mut Task, buf: &mut [u8]) -> io::Result<usize> {
        let mut io = TaskIoTake::new(task, &self.handle);
        io.state().object.read(buf)
    }

    fn read_to_end(&mut self,
                   task: &mut Task,
                   buf: &mut Vec<u8>) -> io::Result<usize> {
        let mut io = TaskIoTake::new(task, &self.handle);
        io.state().object.read_to_end(buf)
    }
}

impl<T> Stream for TaskIoWrite<T>
    where T: Stream<Item=Ready, Error=io::Error>,
{
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        TaskIoTake::new(task, &self.handle).poll(Ready::Write)
    }
}

impl<T> WriteTask for TaskIoWrite<T>
    where T: io::Write + Stream<Item=Ready, Error=io::Error>,
{
    fn write(&mut self, task: &mut Task, buf: &[u8]) -> io::Result<usize> {
        let mut io = TaskIoTake::new(task, &self.handle);
        io.state().object.write(buf)
    }

    fn flush(&mut self, task: &mut Task) -> io::Result<()> {
        let mut io = TaskIoTake::new(task, &self.handle);
        io.state().object.flush()
    }
}
