use std::any::Any;
use std::cell::RefCell;
use std::io;

use futures::{Future, Task, TaskData, Poll, store};
use futures::stream::Stream;

use {WriteTask, ReadTask, Ready};

pub struct TaskIo<T> {
    handle: TaskData<RefCell<Option<State<T>>>>,
}

pub struct TaskIoRead<T> {
    handle: TaskData<RefCell<Option<State<T>>>>,
}

pub struct TaskIoWrite<T> {
    handle: TaskData<RefCell<Option<State<T>>>>,
}

struct State<T> {
    object: T,
    ready: Option<Ready>,
}

impl<T: Any + Send + 'static> TaskIo<T> {
    pub fn new(t: T) -> Box<Future<Item=TaskIo<T>, Error=io::Error>> {
        let state = State {
            object: t,
            ready: None,
        };
        store(RefCell::new(Some(state))).map(|data| {
            TaskIo { handle: data }
        }).boxed()
    }
}

impl<T> TaskIo<T>
    where T: ReadTask + WriteTask,
{
    pub fn split(self) -> (TaskIoRead<T>, TaskIoWrite<T>) {
        (TaskIoRead { handle: self.handle },
         TaskIoWrite { handle: self.handle })
    }
}

struct TaskIoTake<'a, 'b, T: Send + 'static> {
    t: Option<State<T>>,
    task: &'a mut Task,
    handle: &'b TaskData<RefCell<Option<State<T>>>>,
}

impl<'a, 'b, T: Send + 'static> TaskIoTake<'a, 'b, T> {
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

    fn schedule(&mut self, ready: Ready) {
        let state = self.t.as_mut().unwrap();
        if let Some(cur) = state.ready {
            match (ready, cur) {
                (_, Ready::ReadWrite) |
                (Ready::Read, Ready::Read) |
                (Ready::Write, Ready::Write) => return self.task.notify(),

                (Ready::ReadWrite, Ready::Read) |
                (Ready::ReadWrite, Ready::Write) |
                (Ready::Read, Ready::Write) |
                (Ready::Write, Ready::Read) => {}
            }
        }
        state.object.schedule(self.task)
    }
}

impl<'a, 'b, T: Send + 'static> Drop for TaskIoTake<'a, 'b, T> {
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

    fn schedule(&mut self, task: &mut Task) {
        TaskIoTake::new(task, &self.handle).schedule(Ready::ReadWrite)
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

    fn schedule(&mut self, task: &mut Task) {
        TaskIoTake::new(task, &self.handle).schedule(Ready::Read)
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

    fn schedule(&mut self, task: &mut Task) {
        TaskIoTake::new(task, &self.handle).schedule(Ready::Write)
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
