use futures::future::ok;
use futures::stream;
use futures::channel::{oneshot, mpsc};
use futures::task::{self, Wake, Waker};
use futures::executor::block_on;
use futures::sink::SinkErrInto;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::mem;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{Ordering, AtomicBool};

mod support;
use support::*;

#[test]
fn either_sink() {
    let mut s = if true {
        Vec::<i32>::new().left_sink()
    } else {
        VecDeque::<i32>::new().right_sink()
    };

    s.start_send(0).unwrap();
}

#[test]
fn vec_sink() {
    let mut v = Vec::new();
    v.start_send(0).unwrap();
    v.start_send(1).unwrap();
    assert_eq!(v, vec![0, 1]);
    assert_done(move || v.flush(), Ok(vec![0, 1]));
}

#[test]
fn vecdeque_sink() {
    let mut deque = VecDeque::new();
    deque.start_send(2).unwrap();
    deque.start_send(3).unwrap();

    assert_eq!(deque.pop_front(), Some(2));
    assert_eq!(deque.pop_front(), Some(3));
    assert_eq!(deque.pop_front(), None);
}

#[test]
fn send() {
    let v = Vec::new();

    let v = block_on(v.send(0)).unwrap();
    assert_eq!(v, vec![0]);

    let v = block_on(v.send(1)).unwrap();
    assert_eq!(v, vec![0, 1]);

    assert_done(move || v.send(2),
                Ok(vec![0, 1, 2]));
}

#[test]
fn send_all() {
    let v = Vec::new();

    let (v, _) = block_on(v.send_all(stream::iter_ok(vec![0, 1]))).unwrap();
    assert_eq!(v, vec![0, 1]);

    let (v, _) = block_on(v.send_all(stream::iter_ok(vec![2, 3]))).unwrap();
    assert_eq!(v, vec![0, 1, 2, 3]);

    assert_done(
        move || v.send_all(stream::iter_ok(vec![4, 5])).map(|(v, _)| v),
        Ok(vec![0, 1, 2, 3, 4, 5]));
}

// An Unpark struct that records unpark events for inspection
struct Flag(pub AtomicBool);

impl Flag {
    fn new() -> Arc<Flag> {
        Arc::new(Flag(AtomicBool::new(false)))
    }

    fn get(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    fn set(&self, v: bool) {
        self.0.store(v, Ordering::SeqCst)
    }
}

impl Wake for Flag {
    fn wake(arc_self: &Arc<Self>) {
        arc_self.set(true)
    }
}

fn flag_cx<F, R>(f: F) -> R
    where F: FnOnce(Arc<Flag>, &mut Context<'_>) -> R
{
    let flag = Flag::new();
    let map = &mut task::LocalMap::new();
    let waker = Waker::from(flag.clone());
    let exec = &mut support::PanicExec;

    let cx = &mut Context::new(map, &waker, exec);
    f(flag, cx)
}

// Sends a value on an i32 channel sink
struct StartSendFut<S: Sink>(Option<S>, Option<S::SinkItem>);

impl<S: Sink> StartSendFut<S> {
    fn new(sink: S, item: S::SinkItem) -> StartSendFut<S> {
        StartSendFut(Some(sink), Some(item))
    }
}

impl<S: Sink> Future for StartSendFut<S> {
    type Item = S;
    type Error = S::SinkError;

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<S, S::SinkError> {
        {
            let inner = self.0.as_mut().unwrap();
            ready!(inner.poll_ready(cx))?;
            inner.start_send(self.1.take().unwrap())?;
        }
        Ok(Poll::Ready(self.0.take().unwrap()))
    }
}

#[test]
// Test that `start_send` on an `mpsc` channel does indeed block when the
// channel is full
fn mpsc_blocking_start_send() {
    let (mut tx, mut rx) = mpsc::channel::<i32>(0);

    block_on(futures::future::lazy(|_| {
        tx.start_send(0).unwrap();

        flag_cx(|flag, cx| {
            let mut task = StartSendFut::new(tx, 1);

            assert!(task.poll(cx).unwrap().is_pending());
            assert!(!flag.get());
            sassert_next(&mut rx, 0);
            assert!(flag.get());
            flag.set(false);
            assert!(task.poll(cx).unwrap().is_ready());
            assert!(!flag.get());
            sassert_next(&mut rx, 1);

            Ok::<(), ()>(())
        })
    })).unwrap();
}

#[test]
// test `flush` by using `with` to make the first insertion into a sink block
// until a oneshot is completed
fn with_flush() {
    let (tx, rx) = oneshot::channel();
    let mut block = Box::new(rx) as Box<Future<Item = _, Error = _>>;
    let mut sink = Vec::new().with(|elem| {
        mem::replace(&mut block, Box::new(ok(())))
            .map(move |_| elem + 1).map_err(|_| -> Never { panic!() })
    });

    assert_eq!(sink.start_send(0), Ok(()));

    flag_cx(|flag, cx| {
        let mut task = sink.flush();
        assert!(task.poll(cx).unwrap().is_pending());
        tx.send(()).unwrap();
        assert!(flag.get());

        let sink = match task.poll(cx).unwrap() {
            Poll::Ready(sink) => sink,
            _ => panic!()
        };

        assert_eq!(block_on(sink.send(1)).unwrap().get_ref(), &[1, 2]);
    })
}

#[test]
// test simple use of with to change data
fn with_as_map() {
    let sink = Vec::new().with(|item| -> Result<i32, Never> {
        Ok(item * 2)
    });
    let sink = block_on(sink.send(0)).unwrap();
    let sink = block_on(sink.send(1)).unwrap();
    let sink = block_on(sink.send(2)).unwrap();
    assert_eq!(sink.get_ref(), &[0, 2, 4]);
}

#[test]
// test simple use of with_flat_map
fn with_flat_map() {
    let sink = Vec::new().with_flat_map(|item| {
        stream::iter_ok(vec![item; item])
    });
    let sink = block_on(sink.send(0)).unwrap();
    let sink = block_on(sink.send(1)).unwrap();
    let sink = block_on(sink.send(2)).unwrap();
    let sink = block_on(sink.send(3)).unwrap();
    assert_eq!(sink.get_ref(), &[1,2,2,3,3,3]);
}

// Immediately accepts all requests to start pushing, but completion is managed
// by manually flushing
struct ManualFlush<T> {
    data: Vec<T>,
    waiting_tasks: Vec<Waker>,
}

impl<T> Sink for ManualFlush<T> {
    type SinkItem = Option<T>; // Pass None to flush
    type SinkError = ();

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<(), Self::SinkError> {
        Ok(Poll::Ready(()))
    }

    fn start_send(&mut self, f: Self::SinkItem) -> Result<(), Self::SinkError> {
        if let Some(item) = f {
            self.data.push(item);
        } else {
            self.force_flush();
        }
        Ok(())
    }

    fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<(), Self::SinkError> {
        if self.data.is_empty() {
            Ok(Poll::Ready(()))
        } else {
            self.waiting_tasks.push(cx.waker().clone());
            Ok(Poll::Pending)
        }
    }

    fn poll_close(&mut self, cx: &mut Context<'_>) -> Poll<(), Self::SinkError> {
        self.poll_flush(cx)
    }
}

impl<T> ManualFlush<T> {
    fn new() -> ManualFlush<T> {
        ManualFlush {
            data: Vec::new(),
            waiting_tasks: Vec::new()
        }
    }

    fn force_flush(&mut self) -> Vec<T> {
        for task in self.waiting_tasks.drain(..) {
            task.wake()
        }
        mem::replace(&mut self.data, Vec::new())
    }
}

#[test]
// test that the `with` sink doesn't require the underlying sink to flush,
// but doesn't claim to be flushed until the underlying sink is
fn with_flush_propagate() {
    let mut sink = ManualFlush::new().with(|x| -> Result<Option<i32>, ()> { Ok(x) });
    flag_cx(|flag, cx| {
        assert!(sink.poll_ready(cx).unwrap().is_ready());
        sink.start_send(Some(0)).unwrap();
        assert!(sink.poll_ready(cx).unwrap().is_ready());
        sink.start_send(Some(1)).unwrap();

        let mut task = sink.flush();
        assert!(task.poll(cx).unwrap().is_pending());
        assert!(!flag.get());
        assert_eq!(task.get_mut().unwrap().get_mut().force_flush(), vec![0, 1]);
        assert!(flag.get());
        assert!(task.poll(cx).unwrap().is_ready());
    })
}

#[test]
// test that a buffer is a no-nop around a sink that always accepts sends
fn buffer_noop() {
    let sink = Vec::new().buffer(0);
    let sink = block_on(sink.send(0)).unwrap();
    let sink = block_on(sink.send(1)).unwrap();
    assert_eq!(sink.get_ref(), &[0, 1]);

    let sink = Vec::new().buffer(1);
    let sink = block_on(sink.send(0)).unwrap();
    let sink = block_on(sink.send(1)).unwrap();
    assert_eq!(sink.get_ref(), &[0, 1]);
}

struct ManualAllow<T> {
    data: Vec<T>,
    allow: Rc<Allow>,
}

struct Allow {
    flag: Cell<bool>,
    tasks: RefCell<Vec<Waker>>,
}

impl Allow {
    fn new() -> Allow {
        Allow {
            flag: Cell::new(false),
            tasks: RefCell::new(Vec::new()),
        }
    }

    fn check(&self, cx: &mut Context<'_>) -> bool {
        if self.flag.get() {
            true
        } else {
            self.tasks.borrow_mut().push(cx.waker().clone());
            false
        }
    }

    fn start(&self) {
        self.flag.set(true);
        let mut tasks = self.tasks.borrow_mut();
        for task in tasks.drain(..) {
            task.wake();
        }
    }
}

impl<T> Sink for ManualAllow<T> {
    type SinkItem = T;
    type SinkError = Never;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<(), Self::SinkError> {
        if self.allow.check(cx) {
            Ok(Poll::Ready(()))
        } else {
            Ok(Poll::Pending)
        }
    }

    fn start_send(&mut self, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        self.data.push(item);
        Ok(())
    }

    fn poll_flush(&mut self, _: &mut Context<'_>) -> Poll<(), Self::SinkError> {
        Ok(Poll::Ready(()))
    }

    fn poll_close(&mut self, _: &mut Context<'_>) -> Poll<(), Self::SinkError> {
        Ok(Poll::Ready(()))
    }
}

fn manual_allow<T>() -> (ManualAllow<T>, Rc<Allow>) {
    let allow = Rc::new(Allow::new());
    let manual_allow = ManualAllow {
        data: Vec::new(),
        allow: allow.clone(),
    };
    (manual_allow, allow)
}

#[test]
// test basic buffer functionality, including both filling up to capacity,
// and writing out when the underlying sink is ready
fn buffer() {
    let (sink, allow) = manual_allow::<i32>();
    let sink = sink.buffer(2);

    let sink = block_on(StartSendFut::new(sink, 0)).unwrap();
    let sink = block_on(StartSendFut::new(sink, 1)).unwrap();

    flag_cx(|flag, cx| {
        let mut task = sink.send(2);
        assert!(task.poll(cx).unwrap().is_pending());
        assert!(!flag.get());
        allow.start();
        assert!(flag.get());
        match task.poll(cx).unwrap() {
            Poll::Ready(sink) => {
                assert_eq!(sink.get_ref().data, vec![0, 1, 2]);
            }
            _ => panic!()
        }
    })
}

#[test]
fn fanout_smoke() {
    let sink1 = Vec::new();
    let sink2 = Vec::new();
    let sink = sink1.fanout(sink2);
    let stream = futures::stream::iter_ok(vec![1,2,3]);
    let (sink, _) = block_on(sink.send_all(stream)).unwrap();
    let (sink1, sink2) = sink.into_inner();
    assert_eq!(sink1, vec![1,2,3]);
    assert_eq!(sink2, vec![1,2,3]);
}

#[test]
fn fanout_backpressure() {
    let (left_send, left_recv) = mpsc::channel(0);
    let (right_send, right_recv) = mpsc::channel(0);
    let sink = left_send.fanout(right_send);

    let sink = block_on(StartSendFut::new(sink, 0)).unwrap();

    flag_cx(|flag, cx| {
        let mut task = sink.send(2);
        assert!(!flag.get());
        assert!(task.poll(cx).unwrap().is_pending());
        let (item, left_recv) = block_on(left_recv.next()).unwrap();
        assert_eq!(item, Some(0));
        assert!(flag.get());
        assert!(task.poll(cx).unwrap().is_pending());
        let (item, right_recv) = block_on(right_recv.next()).unwrap();
        assert_eq!(item, Some(0));
        assert!(flag.get());
        assert!(task.poll(cx).unwrap().is_ready());
        // make sure receivers live until end of test to prevent send errors
        drop(left_recv);
        drop(right_recv);
    })
}

#[test]
fn map_err() {
    panic_waker_cx(|cx| {
        let (tx, _rx) = mpsc::channel(1);
        let mut tx = tx.sink_map_err(|_| ());
        assert_eq!(tx.start_send(()), Ok(()));
        assert_eq!(tx.poll_flush(cx), Ok(Poll::Ready(())));
    });

    let tx = mpsc::channel(0).0;
    assert_eq!(tx.sink_map_err(|_| ()).start_send(()), Err(()));
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct FromErrTest;

impl From<mpsc::SendError> for FromErrTest {
    fn from(_: mpsc::SendError) -> FromErrTest {
        FromErrTest
    }
}

#[test]
fn from_err() {
    panic_waker_cx(|cx| {
        let (tx, _rx) = mpsc::channel(1);
        let mut tx: SinkErrInto<mpsc::Sender<()>, FromErrTest> = tx.sink_err_into();
        assert_eq!(tx.start_send(()), Ok(()));
        assert_eq!(tx.poll_flush(cx), Ok(Poll::Ready(())));
    });

    let tx = mpsc::channel(0).0;
    assert_eq!(tx.sink_err_into().start_send(()), Err(FromErrTest));
}
