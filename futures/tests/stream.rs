use std::{
    cell::{Cell, RefCell},
    convert::Infallible,
    iter,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    task::{Context, Waker},
};

use futures::{
    FutureExt,
    channel::mpsc,
    executor::block_on,
    future::{self, FusedFuture, Future},
    lock::Mutex,
    ready,
    sink::SinkExt,
    stream::{self, Forward, StreamExt},
    task::Poll,
};
use futures_core::Stream;
use futures_executor::ThreadPool;
use futures_sink::Sink;
use futures_test::task::{new_count_waker, noop_context};

struct CountedStream {
    next: usize,
    end: usize,
    polls: Rc<Cell<usize>>,
}

impl Stream for CountedStream {
    type Item = usize;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.polls.set(self.polls.get() + 1);
        if self.next == self.end {
            Poll::Ready(None)
        } else {
            let item = self.next;
            self.next += 1;
            Poll::Ready(Some(item))
        }
    }
}

struct RecordingSink {
    items: Rc<RefCell<Vec<usize>>>,
    closed: Rc<Cell<bool>>,
}

impl Sink<usize> for RecordingSink {
    type Error = Infallible;

    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: usize) -> Result<(), Self::Error> {
        self.items.borrow_mut().push(item);
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.closed.set(true);
        Poll::Ready(Ok(()))
    }
}

fn counted_forward(
    end: usize,
) -> (Forward<CountedStream, RecordingSink>, Rc<Cell<usize>>, Rc<RefCell<Vec<usize>>>, Rc<Cell<bool>>)
{
    let polls = Rc::new(Cell::new(0));
    let items = Rc::new(RefCell::new(Vec::new()));
    let closed = Rc::new(Cell::new(false));
    let stream = CountedStream { next: 0, end, polls: polls.clone() };
    let sink = RecordingSink { items: items.clone(), closed: closed.clone() };
    (stream.forward(sink), polls, items, closed)
}

#[test]
fn forward_ready_stream_returns_control_before_source_exhaustion() {
    let (mut forward, polls, items, closed) = counted_forward(96);
    let (waker, wakes) = new_count_waker();
    let mut context = Context::from_waker(&waker);

    let first_poll = Pin::new(&mut forward).poll(&mut context);
    assert!(matches!(first_poll, Poll::Pending), "first poll: {:?}", first_poll);
    assert!(polls.get() > 0);
    assert!(polls.get() <= 32);
    let accepted = items.borrow().clone();
    assert!(accepted.iter().copied().eq(0..accepted.len()));
    assert!(!closed.get());
    assert!(wakes.get() > 0);

    let mut result = Poll::Pending;
    for _ in 0..8 {
        result = Pin::new(&mut forward).poll(&mut context);
        if result.is_ready() {
            break;
        }
    }
    assert!(matches!(result, Poll::Ready(Ok(()))), "final poll: {:?}", result);
    assert_eq!(*items.borrow(), (0..96).collect::<Vec<_>>());
    assert!(closed.get());
    assert!(forward.is_terminated());
}

#[test]
fn forward_finite_ready_controls_complete_in_order() {
    for end in [0, 1, 32, 33] {
        let (mut forward, polls, items, closed) = counted_forward(end);
        let (waker, _) = new_count_waker();
        let mut context = Context::from_waker(&waker);

        if end <= 1 {
            let first_poll = Pin::new(&mut forward).poll(&mut context);
            assert!(matches!(first_poll, Poll::Ready(Ok(()))), "first poll: {:?}", first_poll);
            assert_eq!(*items.borrow(), (0..end).collect::<Vec<_>>());
            assert!(closed.get());
            assert!(forward.is_terminated());
            continue;
        }

        let mut result = Poll::Pending;

        for _ in 0..8 {
            result = Pin::new(&mut forward).poll(&mut context);
            if result.is_ready() {
                break;
            }
        }

        assert!(matches!(result, Poll::Ready(Ok(()))));
        assert_eq!(*items.borrow(), (0..end).collect::<Vec<_>>());
        assert!(closed.get());
        assert!(polls.get() <= end + 1);
        assert!(forward.is_terminated());
    }
}

struct PendingReadySink {
    pending: bool,
    items: Rc<RefCell<Vec<usize>>>,
}

impl Sink<usize> for PendingReadySink {
    type Error = Infallible;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.pending {
            self.pending = false;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn start_send(self: Pin<&mut Self>, item: usize) -> Result<(), Self::Error> {
        self.items.borrow_mut().push(item);
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

#[test]
fn forward_preserves_buffered_item_when_sink_is_not_ready() {
    let items = Rc::new(RefCell::new(Vec::new()));
    let stream = CountedStream { next: 0, end: 1, polls: Rc::new(Cell::new(0)) };
    let polls = stream.polls.clone();
    let sink = PendingReadySink { pending: true, items: items.clone() };
    let mut forward = stream.forward(sink);
    let mut context = noop_context();

    assert!(matches!(Pin::new(&mut forward).poll(&mut context), Poll::Pending));
    assert_eq!(polls.get(), 1);
    assert!(items.borrow().is_empty());
    assert!(matches!(Pin::new(&mut forward).poll(&mut context), Poll::Ready(Ok(()))));
    assert_eq!(*items.borrow(), vec![0]);
}

struct PendingStream {
    state: usize,
}

impl Stream for PendingStream {
    type Item = usize;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.state {
            0 => {
                self.state = 1;
                Poll::Ready(Some(0))
            }
            1 => {
                self.state = 2;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            _ => Poll::Ready(None),
        }
    }
}

struct ErrorStream {
    next: usize,
    end: usize,
    pending: bool,
    events: Rc<RefCell<Vec<&'static str>>>,
}

impl Stream for ErrorStream {
    type Item = usize;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.events.borrow_mut().push("source");
        if self.next == self.end {
            if self.pending {
                self.pending = false;
                Poll::Pending
            } else {
                Poll::Ready(None)
            }
        } else {
            let item = self.next;
            self.next += 1;
            Poll::Ready(Some(item))
        }
    }
}

#[test]
fn forward_keeps_natural_stream_pending_path() {
    let items = Rc::new(RefCell::new(Vec::new()));
    let flushes = Rc::new(Cell::new(0));
    let sink = CountingFlushSink { items: items.clone(), flushes: flushes.clone() };
    let mut forward = PendingStream { state: 0 }.forward(sink);
    let (waker, wakes) = new_count_waker();
    let mut context = Context::from_waker(&waker);

    assert!(matches!(Pin::new(&mut forward).poll(&mut context), Poll::Pending));
    assert_eq!(*items.borrow(), vec![0]);
    assert_eq!(flushes.get(), 1);
    assert_eq!(wakes.get(), 1);
    assert!(matches!(Pin::new(&mut forward).poll(&mut context), Poll::Ready(Ok(()))));
}

struct CountingFlushSink {
    items: Rc<RefCell<Vec<usize>>>,
    flushes: Rc<Cell<usize>>,
}

impl Sink<usize> for CountingFlushSink {
    type Error = Infallible;

    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: usize) -> Result<(), Self::Error> {
        self.items.borrow_mut().push(item);
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.flushes.set(self.flushes.get() + 1);
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

#[derive(Clone, Copy)]
enum Failure {
    Ready,
    StartSend,
    Flush,
    Close,
}

struct FailingSink {
    failure: Failure,
    events: Rc<RefCell<Vec<&'static str>>>,
}

impl Sink<usize> for FailingSink {
    type Error = &'static str;

    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.events.borrow_mut().push("ready");
        if matches!(self.failure, Failure::Ready) {
            Poll::Ready(Err("ready"))
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn start_send(self: Pin<&mut Self>, _: usize) -> Result<(), Self::Error> {
        self.events.borrow_mut().push("start_send");
        if matches!(self.failure, Failure::StartSend) { Err("start_send") } else { Ok(()) }
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.events.borrow_mut().push("flush");
        if matches!(self.failure, Failure::Flush) {
            Poll::Ready(Err("flush"))
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.events.borrow_mut().push("close");
        if matches!(self.failure, Failure::Close) {
            Poll::Ready(Err("close"))
        } else {
            Poll::Ready(Ok(()))
        }
    }
}

#[test]
fn forward_preserves_sink_errors() {
    let mut context = noop_context();
    let events = Rc::new(RefCell::new(Vec::new()));

    let mut ready = ErrorStream { next: 0, end: 1, pending: false, events: events.clone() }
        .forward(FailingSink { failure: Failure::Ready, events: events.clone() });
    assert!(matches!(Pin::new(&mut ready).poll(&mut context), Poll::Ready(Err("ready"))));
    assert_eq!(*events.borrow(), vec!["source", "ready"]);
    assert!(!ready.is_terminated());

    events.borrow_mut().clear();
    let mut start_send = ErrorStream { next: 0, end: 1, pending: false, events: events.clone() }
        .forward(FailingSink { failure: Failure::StartSend, events: events.clone() });
    assert!(matches!(Pin::new(&mut start_send).poll(&mut context), Poll::Ready(Err("start_send"))));
    assert_eq!(*events.borrow(), vec!["source", "ready", "start_send"]);
    assert!(!start_send.is_terminated());

    events.borrow_mut().clear();
    let mut flush = ErrorStream { next: 0, end: 1, pending: true, events: events.clone() }
        .forward(FailingSink { failure: Failure::Flush, events: events.clone() });
    assert!(matches!(Pin::new(&mut flush).poll(&mut context), Poll::Ready(Err("flush"))));
    assert_eq!(*events.borrow(), vec!["source", "ready", "start_send", "source", "flush"]);
    assert!(!flush.is_terminated());

    events.borrow_mut().clear();
    let mut close = ErrorStream { next: 0, end: 0, pending: false, events: events.clone() }
        .forward(FailingSink { failure: Failure::Close, events: events.clone() });
    assert!(matches!(Pin::new(&mut close).poll(&mut context), Poll::Ready(Err("close"))));
    assert_eq!(*events.borrow(), vec!["source", "close"]);
    assert!(!close.is_terminated());
}

#[test]
fn forward_propagates_artificial_flush_error() {
    let mut context = noop_context();
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut artificial_flush =
        ErrorStream { next: 0, end: 32, pending: false, events: events.clone() }
            .forward(FailingSink { failure: Failure::Flush, events: events.clone() });
    assert!(matches!(
        Pin::new(&mut artificial_flush).poll(&mut context),
        Poll::Ready(Err("flush"))
    ));
    assert_eq!(events.borrow().last(), Some(&"flush"));
    assert_eq!(events.borrow().iter().filter(|&&event| event == "flush").count(), 1);
    assert_eq!(events.borrow().iter().filter(|&&event| event == "source").count(), 32);
    assert!(!artificial_flush.is_terminated());
}

#[test]
fn forward_preservation_controls() {
    forward_preserves_sink_errors();
    forward_preserves_buffered_item_when_sink_is_not_ready();
    forward_keeps_natural_stream_pending_path();
    forward_does_not_repoll_stream_while_close_is_pending();
}

struct PendingFlushSink {
    items: Rc<RefCell<Vec<usize>>>,
    pending: bool,
    waiting_waker: Rc<RefCell<Option<Waker>>>,
}

impl Sink<usize> for PendingFlushSink {
    type Error = Infallible;

    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: usize) -> Result<(), Self::Error> {
        self.items.borrow_mut().push(item);
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.pending {
            self.pending = false;
            *self.waiting_waker.borrow_mut() = Some(cx.waker().clone());
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

#[test]
fn forward_waits_for_artificial_flush_wake() {
    let items = Rc::new(RefCell::new(Vec::new()));
    let waiting_waker = Rc::new(RefCell::new(None));
    let polls = Rc::new(Cell::new(0));
    let stream = CountedStream { next: 0, end: 96, polls: polls.clone() };
    let sink = PendingFlushSink {
        items: items.clone(),
        pending: true,
        waiting_waker: waiting_waker.clone(),
    };
    let mut forward = stream.forward(sink);
    let (waker, wakes) = new_count_waker();
    let mut context = Context::from_waker(&waker);

    assert!(matches!(Pin::new(&mut forward).poll(&mut context), Poll::Pending));
    let accepted = items.borrow().clone();
    assert!(polls.get() > 0);
    assert!(polls.get() <= 32);
    assert!(accepted.len() <= polls.get());
    assert!(accepted.iter().copied().eq(0..accepted.len()));
    assert!(polls.get() <= 32);
    assert_eq!(wakes.get(), 0);
    waiting_waker.borrow_mut().take().expect("flush waker").wake();
    assert_eq!(wakes.get(), 1);

    let mut result = Poll::Pending;
    for _ in 0..8 {
        result = Pin::new(&mut forward).poll(&mut context);
        if result.is_ready() {
            break;
        }
    }
    assert!(matches!(result, Poll::Ready(Ok(()))));
    assert_eq!(*items.borrow(), (0..96).collect::<Vec<_>>());
}

struct PendingCloseSink {
    pending: bool,
}

impl Sink<usize> for PendingCloseSink {
    type Error = Infallible;

    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.pending {
            self.pending = false;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }
}

#[test]
fn forward_does_not_repoll_stream_while_close_is_pending() {
    let polls = Rc::new(Cell::new(0));
    let stream = CountedStream { next: 0, end: 0, polls: polls.clone() };
    let mut forward = stream.forward(PendingCloseSink { pending: true });
    let mut context = noop_context();

    assert!(matches!(Pin::new(&mut forward).poll(&mut context), Poll::Pending));
    assert_eq!(polls.get(), 1);
    assert!(matches!(Pin::new(&mut forward).poll(&mut context), Poll::Ready(Ok(()))));
    assert_eq!(polls.get(), 1);
}

#[test]
fn select() {
    fn select_and_compare(a: Vec<u32>, b: Vec<u32>, expected: Vec<u32>) {
        let a = stream::iter(a);
        let b = stream::iter(b);
        let vec = block_on(stream::select(a, b).collect::<Vec<_>>());
        assert_eq!(vec, expected);
    }

    select_and_compare(vec![1, 2, 3], vec![4, 5, 6], vec![1, 4, 2, 5, 3, 6]);
    select_and_compare(vec![1, 2, 3], vec![4, 5], vec![1, 4, 2, 5, 3]);
    select_and_compare(vec![1, 2], vec![4, 5, 6], vec![1, 4, 2, 5, 6]);
}

#[test]
fn flat_map() {
    block_on(async {
        let st =
            stream::iter(vec![stream::iter(0..=4u8), stream::iter(6..=10), stream::iter(0..=2)]);

        let values: Vec<_> =
            st.flat_map(|s| s.filter(|v| futures::future::ready(v % 2 == 0))).collect().await;

        assert_eq!(values, vec![0, 2, 4, 6, 8, 10, 0, 2]);
    });
}

#[test]
fn scan() {
    block_on(async {
        let values = stream::iter(vec![1u8, 2, 3, 4, 6, 8, 2])
            .scan(1, |mut state, e| async move {
                state += 1;
                if e < state { Some((state, e)) } else { None }
            })
            .collect::<Vec<_>>()
            .await;

        assert_eq!(values, vec![1u8, 2, 3, 4]);
    });

    block_on(async {
        let mut state = vec![];
        let values = stream::iter(vec![1u8, 2, 3, 4, 6, 8, 2])
            .scan(&mut state, |state, e| async move {
                state.push(e);
                Some((state, e))
            })
            .collect::<Vec<_>>()
            .await;

        assert_eq!(values, state);
    });
}

#[test]
fn flatten_unordered() {
    use std::{
        convert::identity,
        pin::Pin,
        sync::atomic::{AtomicBool, Ordering},
        thread,
        time::Duration,
    };

    use futures::{executor::block_on, stream::*, task::*};

    struct DataStream {
        data: Vec<u8>,
        polled: bool,
        wake_immediately: bool,
    }

    impl Stream for DataStream {
        type Item = u8;

        fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            if !self.polled {
                if !self.wake_immediately {
                    let waker = ctx.waker().clone();
                    let sleep_time =
                        Duration::from_millis(*self.data.first().unwrap_or(&0) as u64 / 10);
                    thread::spawn(move || {
                        thread::sleep(sleep_time);
                        waker.wake_by_ref();
                    });
                } else {
                    ctx.waker().wake_by_ref();
                }
                self.polled = true;
                Poll::Pending
            } else {
                self.polled = false;
                Poll::Ready(self.data.pop())
            }
        }
    }

    struct Interchanger {
        polled: bool,
        base: u8,
        wake_immediately: bool,
    }

    impl Stream for Interchanger {
        type Item = DataStream;

        fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            if !self.polled {
                self.polled = true;
                if !self.wake_immediately {
                    let waker = ctx.waker().clone();
                    let sleep_time = Duration::from_millis(self.base as u64);
                    thread::spawn(move || {
                        thread::sleep(sleep_time);
                        waker.wake_by_ref();
                    });
                } else {
                    ctx.waker().wake_by_ref();
                }
                Poll::Pending
            } else {
                let data: Vec<_> = (0..6).rev().map(|v| v + self.base * 6).collect();
                self.base += 1;
                self.polled = false;
                Poll::Ready(Some(DataStream {
                    polled: false,
                    data,
                    wake_immediately: self.wake_immediately && self.base % 2 == 0,
                }))
            }
        }
    }

    // basic behaviour
    {
        block_on(async {
            let st = stream::iter(vec![
                stream::iter(0..=4u8),
                stream::iter(6..=10),
                stream::iter(10..=12),
            ]);

            let fl_unordered = st.flatten_unordered(3).collect::<Vec<_>>().await;

            assert_eq!(fl_unordered, vec![0, 6, 10, 1, 7, 11, 2, 8, 12, 3, 9, 4, 10]);
        });

        block_on(async {
            let st = stream::iter(vec![
                stream::iter(0..=4u8),
                stream::iter(6..=10),
                stream::iter(0..=2),
            ]);

            let mut fm_unordered = st
                .flat_map_unordered(1, |s| s.filter(|v| futures::future::ready(v % 2 == 0)))
                .collect::<Vec<_>>()
                .await;

            fm_unordered.sort_unstable();

            assert_eq!(fm_unordered, vec![0, 0, 2, 2, 4, 6, 8, 10]);
        });
    }

    // wake up immediately
    {
        block_on(async {
            let mut fl_unordered = Interchanger { polled: false, base: 0, wake_immediately: true }
                .take(10)
                .map(|s| s.map(identity))
                .flatten_unordered(10)
                .collect::<Vec<_>>()
                .await;

            fl_unordered.sort_unstable();

            assert_eq!(fl_unordered, (0..60).collect::<Vec<u8>>());
        });

        block_on(async {
            let mut fm_unordered = Interchanger { polled: false, base: 0, wake_immediately: true }
                .take(10)
                .flat_map_unordered(10, |s| s.map(identity))
                .collect::<Vec<_>>()
                .await;

            fm_unordered.sort_unstable();

            assert_eq!(fm_unordered, (0..60).collect::<Vec<u8>>());
        });
    }

    // wake up after delay
    {
        block_on(async {
            let mut fl_unordered = Interchanger { polled: false, base: 0, wake_immediately: false }
                .take(10)
                .map(|s| s.map(identity))
                .flatten_unordered(10)
                .collect::<Vec<_>>()
                .await;

            fl_unordered.sort_unstable();

            assert_eq!(fl_unordered, (0..60).collect::<Vec<u8>>());
        });

        block_on(async {
            let mut fm_unordered = Interchanger { polled: false, base: 0, wake_immediately: false }
                .take(10)
                .flat_map_unordered(10, |s| s.map(identity))
                .collect::<Vec<_>>()
                .await;

            fm_unordered.sort_unstable();

            assert_eq!(fm_unordered, (0..60).collect::<Vec<u8>>());
        });

        block_on(async {
            let (mut fm_unordered, mut fl_unordered) = futures_util::join!(
                Interchanger { polled: false, base: 0, wake_immediately: false }
                    .take(10)
                    .flat_map_unordered(10, |s| s.map(identity))
                    .collect::<Vec<_>>(),
                Interchanger { polled: false, base: 0, wake_immediately: false }
                    .take(10)
                    .map(|s| s.map(identity))
                    .flatten_unordered(10)
                    .collect::<Vec<_>>()
            );

            fm_unordered.sort_unstable();
            fl_unordered.sort_unstable();

            assert_eq!(fm_unordered, fl_unordered);
            assert_eq!(fm_unordered, (0..60).collect::<Vec<u8>>());
        });
    }

    // waker panics
    {
        let stream = Arc::new(Mutex::new(
            Interchanger { polled: false, base: 0, wake_immediately: true }
                .take(10)
                .flat_map_unordered(10, |s| s.map(identity)),
        ));

        struct PanicWaker;

        impl ArcWake for PanicWaker {
            fn wake_by_ref(_arc_self: &Arc<Self>) {
                panic!("WAKE UP");
            }
        }

        std::thread::spawn({
            let stream = stream.clone();
            move || {
                let mut st = poll_fn(|cx| {
                    let mut lock = ready!(stream.lock().poll_unpin(cx));

                    let panic_waker = waker(Arc::new(PanicWaker));
                    let mut panic_cx = Context::from_waker(&panic_waker);
                    let _ = ready!(lock.poll_next_unpin(&mut panic_cx));

                    Poll::Ready(Some(()))
                });

                block_on(st.next())
            }
        })
        .join()
        .unwrap_err();

        block_on(async move {
            let mut values: Vec<_> = stream.lock().await.by_ref().collect().await;
            values.sort_unstable();

            assert_eq!(values, (0..60).collect::<Vec<u8>>());
        });
    }

    // stream panics
    {
        let st = stream::iter(iter::once(
            once(Box::pin(async { panic!("Polled") })).left_stream::<DataStream>(),
        ))
        .chain(
            Interchanger { polled: false, base: 0, wake_immediately: true }
                .map(|stream| stream.right_stream())
                .take(10),
        );

        let stream = Arc::new(Mutex::new(st.flatten_unordered(10)));

        std::thread::spawn({
            let stream = stream.clone();
            move || {
                let mut st = poll_fn(|cx| {
                    let mut lock = ready!(stream.lock().poll_unpin(cx));
                    let data = ready!(lock.poll_next_unpin(cx));

                    Poll::Ready(data)
                });

                block_on(st.next())
            }
        })
        .join()
        .unwrap_err();

        block_on(async move {
            let mut values: Vec<_> = stream.lock().await.by_ref().collect().await;
            values.sort_unstable();

            assert_eq!(values, (0..60).collect::<Vec<u8>>());
        });
    }

    fn timeout<I: Clone>(time: Duration, value: I) -> impl Future<Output = I> {
        let ready = Arc::new(AtomicBool::new(false));
        let mut spawned = false;

        future::poll_fn(move |cx| {
            if !spawned {
                let waker = cx.waker().clone();
                let ready = ready.clone();

                std::thread::spawn(move || {
                    std::thread::sleep(time);
                    ready.store(true, Ordering::Release);

                    waker.wake_by_ref()
                });
                spawned = true;
            }

            if ready.load(Ordering::Acquire) { Poll::Ready(value.clone()) } else { Poll::Pending }
        })
    }

    fn build_nested_fu<S: Stream + Unpin>(st: S) -> impl Stream<Item = S::Item> + Unpin
    where
        S::Item: Clone,
    {
        let inner = st
            .then(|item| timeout(Duration::from_millis(50), item))
            .enumerate()
            .map(|(idx, value)| {
                stream::once(if idx % 2 == 0 {
                    future::ready(value).left_future()
                } else {
                    timeout(Duration::from_millis(100), value).right_future()
                })
            })
            .flatten_unordered(None);

        stream::once(future::ready(inner)).flatten_unordered(None)
    }

    // nested `flatten_unordered`
    let te = ThreadPool::new().unwrap();
    let base_handle = te
        .spawn_with_handle(async move {
            let fu = build_nested_fu(stream::iter(1..=10));

            assert_eq!(fu.count().await, 10);
        })
        .unwrap();

    block_on(base_handle);

    let empty_state_move_handle = te
        .spawn_with_handle(async move {
            let mut fu = build_nested_fu(stream::iter(1..10));
            {
                let mut cx = noop_context();
                let _ = fu.poll_next_unpin(&mut cx);
                let _ = fu.poll_next_unpin(&mut cx);
            }

            assert_eq!(fu.count().await, 9);
        })
        .unwrap();

    block_on(empty_state_move_handle);
}

#[test]
fn take_until() {
    fn make_stop_fut(stop_on: u32) -> impl Future<Output = ()> {
        let mut i = 0;
        future::poll_fn(move |_cx| {
            i += 1;
            if i <= stop_on { Poll::Pending } else { Poll::Ready(()) }
        })
    }

    block_on(async {
        // Verify stopping works:
        let stream = stream::iter(1u32..=10);
        let stop_fut = make_stop_fut(5);

        let stream = stream.take_until(stop_fut);
        let last = stream.fold(0, |_, i| async move { i }).await;
        assert_eq!(last, 5);

        // Verify take_future() works:
        let stream = stream::iter(1..=10);
        let stop_fut = make_stop_fut(5);

        let mut stream = stream.take_until(stop_fut);

        assert_eq!(stream.next().await, Some(1));
        assert_eq!(stream.next().await, Some(2));

        stream.take_future();

        let last = stream.fold(0, |_, i| async move { i }).await;
        assert_eq!(last, 10);

        // Verify take_future() returns None if stream is stopped:
        let stream = stream::iter(1u32..=10);
        let stop_fut = make_stop_fut(1);
        let mut stream = stream.take_until(stop_fut);
        assert_eq!(stream.next().await, Some(1));
        assert_eq!(stream.next().await, None);
        assert!(stream.take_future().is_none());

        // Verify TakeUntil is fused:
        let mut i = 0;
        let stream = stream::poll_fn(move |_cx| {
            i += 1;
            match i {
                1 => Poll::Ready(Some(1)),
                2 => Poll::Ready(None),
                _ => panic!("TakeUntil not fused"),
            }
        });

        let stop_fut = make_stop_fut(1);
        let mut stream = stream.take_until(stop_fut);
        assert_eq!(stream.next().await, Some(1));
        assert_eq!(stream.next().await, None);
        assert_eq!(stream.next().await, None);
    });
}

#[test]
#[should_panic]
fn chunks_panic_on_cap_zero() {
    let (_, rx1) = mpsc::channel::<()>(1);

    let _ = rx1.chunks(0);
}

#[test]
#[should_panic]
fn ready_chunks_panic_on_cap_zero() {
    let (_, rx1) = mpsc::channel::<()>(1);

    let _ = rx1.ready_chunks(0);
}

#[test]
fn ready_chunks() {
    let (mut tx, rx1) = mpsc::channel::<i32>(16);

    let mut s = rx1.ready_chunks(2);

    let mut cx = noop_context();
    assert!(s.next().poll_unpin(&mut cx).is_pending());

    block_on(async {
        tx.send(1).await.unwrap();

        assert_eq!(s.next().await.unwrap(), vec![1]);
        tx.send(2).await.unwrap();
        tx.send(3).await.unwrap();
        tx.send(4).await.unwrap();
        assert_eq!(s.next().await.unwrap(), vec![2, 3]);
        assert_eq!(s.next().await.unwrap(), vec![4]);
    });
}

struct SlowStream {
    times_should_poll: usize,
    times_polled: Rc<Cell<usize>>,
}
impl Stream for SlowStream {
    type Item = usize;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.times_polled.set(self.times_polled.get() + 1);
        if self.times_polled.get() % 2 == 0 {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        if self.times_polled.get() >= self.times_should_poll {
            return Poll::Ready(None);
        }
        Poll::Ready(Some(self.times_polled.get()))
    }
}

#[test]
fn select_with_strategy_doesnt_terminate_early() {
    for side in [stream::PollNext::Left, stream::PollNext::Right] {
        let times_should_poll = 10;
        let count = Rc::new(Cell::new(0));
        let b = stream::iter([10, 20]);

        let mut selected = stream::select_with_strategy(
            SlowStream { times_should_poll, times_polled: count.clone() },
            b,
            |_: &mut ()| side,
        );
        block_on(async move { while selected.next().await.is_some() {} });
        assert_eq!(count.get(), times_should_poll + 1);
    }
}

async fn is_even(number: u8) -> bool {
    number % 2 == 0
}

#[test]
fn all() {
    block_on(async {
        let empty: [u8; 0] = [];
        let st = stream::iter(empty);
        let all = st.all(is_even).await;
        assert!(all);

        let st = stream::iter([2, 4, 6, 8]);
        let all = st.all(is_even).await;
        assert!(all);

        let st = stream::iter([2, 3, 4]);
        let all = st.all(is_even).await;
        assert!(!all);
    });
}

#[test]
fn any() {
    block_on(async {
        let empty: [u8; 0] = [];
        let st = stream::iter(empty);
        let any = st.any(is_even).await;
        assert!(!any);

        let st = stream::iter([1, 2, 3]);
        let any = st.any(is_even).await;
        assert!(any);

        let st = stream::iter([1, 3, 5]);
        let any = st.any(is_even).await;
        assert!(!any);
    });
}
