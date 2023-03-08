use futures::channel::mpsc;
use futures::executor::{block_on, block_on_stream};
use futures::future;
use futures::sink::SinkExt;
use futures::stream::{self, LocalBoxStream, StreamExt};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::panic::AssertUnwindSafe;
use std::thread;
use std::task::Poll;

struct CountClone(Rc<Cell<i32>>);

impl Clone for CountClone {
    fn clone(&self) -> Self {
        self.0.set(self.0.get() + 1);
        Self(self.0.clone())
    }
}

fn send_shared_and_wait_on_multiple_threads(threads_number: u32) {
    let items = [6, 9, 200, 3, 4, 192, 54];
    let (mut tx, rx) = mpsc::channel::<i32>(3);
    let join_handles = {
        let s = rx.shared(4);
        (0..threads_number)
            .map(|_| {
                let mut cloned_stream = s.clone();
                thread::spawn(move || {
                    for i in items {
                        assert_eq!(block_on(cloned_stream.next()).unwrap(), i);
                    }
                })
            })
            .collect::<Vec<_>>()
    };

    for i in items {
        block_on(tx.send(i)).unwrap();
    }
    tx.close_channel();

    for join_handle in join_handles {
        join_handle.join().unwrap();
    }
}

#[test]
fn one_thread() {
    send_shared_and_wait_on_multiple_threads(1);
}

#[test]
fn two_threads() {
    send_shared_and_wait_on_multiple_threads(2);
}

#[test]
fn many_threads() {
    send_shared_and_wait_on_multiple_threads(1000);
}

#[test]
fn drop_on_one_task_ok() {
    let (mut tx, rx) = mpsc::channel::<u32>(2);
    let s1 = rx.shared(2);
    let s2 = s1.clone();

    let (mut tx2, rx2) = mpsc::channel::<u32>(2);

    let t1 = thread::spawn(|| {
        let f = stream::select(s1, rx2).take(2).collect::<Vec<_>>();
        drop(block_on(f));
    });

    let (tx3, rx3) = mpsc::channel::<u32>(2);
    let t2 = thread::spawn(move || {
        let _ = block_on(s2.forward(tx3));
    });

    block_on(tx.send(42)).unwrap();
    block_on(tx2.send(11)).unwrap(); // cancel s1
    drop(tx2);
    t1.join().unwrap();

    block_on(tx.send(43)).unwrap();
    drop(tx);
    let result: Vec<_> = block_on_stream(rx3).collect();
    assert_eq!(result, [42, 43]);
    t2.join().unwrap();
}

#[test]
fn drop_in_poll() {
    let slot1 = Rc::new(RefCell::new(None));
    let slot2 = slot1.clone();

    let mut stream1 = stream::once(future::lazy(move |_| {
        slot2.replace(None);
        1
    }))
    .shared(1);

    let stream2: LocalBoxStream<_> = Box::pin(stream1.clone());
    slot1.replace(Some(stream2));

    assert_eq!(block_on(stream1.next()), Some(1));
}

#[test]
fn dont_clone_in_single_owner_shared_stream() {
    let counter = CountClone(Rc::new(Cell::new(0)));
    let (mut tx, rx) = mpsc::channel(2);

    let mut rx = rx.shared(1);

    block_on(tx.send(counter)).unwrap();

    assert_eq!(block_on(rx.next()).unwrap().0.get(), 0);
}

#[test]
fn dont_do_unnecessary_clones_on_output() {
    let counter = CountClone(Rc::new(Cell::new(0)));
    let (mut tx, rx) = mpsc::channel(2);

    let mut rx = rx.shared(1);

    block_on(tx.send(counter)).unwrap();

    assert_eq!(block_on(rx.clone().next()).unwrap().0.get(), 1);
    assert_eq!(block_on(rx.clone().next()).unwrap().0.get(), 2);
    assert_eq!(block_on(rx.next()).unwrap().0.get(), 2);
}

#[test]
fn shared_stream_that_wakes_itself_until_pending_is_returned() {
    let proceed = Cell::new(false);
    let mut stream = stream::poll_fn(|cx| {
        if proceed.get() {
            Poll::Ready(Some(()))
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .shared(3);

    assert_eq!(block_on(future::join(stream.next(), async { proceed.set(true) })), (Some(()), ()));
}

#[test]
#[should_panic(expected = "inner stream panicked during poll")]
fn panic_while_poll() {
    let mut stream = stream::poll_fn::<i8, _>(|_| panic!("test")).shared(1);

    let mut stream_captured = stream.clone();
    std::panic::catch_unwind(AssertUnwindSafe(|| {
        block_on(stream_captured.next());
    })).unwrap_err();

    block_on(stream.next());
}

#[test]
#[should_panic(expected = "test_marker")]
fn poll_while_panic() {
    struct S;

    impl Drop for S {
        fn drop(&mut self) {
            let mut stream = stream::repeat(1).shared(2);
            assert_eq!(block_on(stream.clone().next()), Some(1));
            assert_eq!(block_on(stream.next()), Some(1));
        }
    }

    let _s = S {};

    panic!("test_marker");
}

