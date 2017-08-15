#![cfg(feature = "use_std")]

extern crate futures;

use futures::{Future, Stream, Sink, Async, AsyncSink};
use futures::future::{lazy, ok};
use futures::stream::unfold;
use futures::sync::mpsc;

use std::time::Duration;
use std::thread;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

mod support;
use support::*;


trait AssertSend: Send {}
impl AssertSend for mpsc::Sender<i32> {}
impl AssertSend for mpsc::Receiver<i32> {}

#[test]
fn send_recv() {
    let (tx, rx) = mpsc::channel::<i32>(16);
    let mut rx = rx.wait();

    tx.send(1).wait().unwrap();

    assert_eq!(rx.next().unwrap(), Ok(1));
}

#[test]
fn send_recv_no_buffer() {
    let (mut tx, mut rx) = mpsc::channel::<i32>(0);

    // Run on a task context
    lazy(move || {
        assert!(tx.poll_complete().unwrap().is_ready());

        // Send first message

        let res = tx.start_send(1).unwrap();
        assert!(is_ready(&res));

        // Send second message
        let res = tx.start_send(2).unwrap();
        assert!(!is_ready(&res));

        // Take the value
        assert_eq!(rx.poll().unwrap(), Async::Ready(Some(1)));

        let res = tx.start_send(2).unwrap();
        assert!(is_ready(&res));

        // Take the value
        assert_eq!(rx.poll().unwrap(), Async::Ready(Some(2)));

        Ok::<(), ()>(())
    }).wait().unwrap();
}

#[test]
fn send_shared_recv() {
    let (tx1, rx) = mpsc::channel::<i32>(16);
    let tx2 = tx1.clone();
    let mut rx = rx.wait();

    tx1.send(1).wait().unwrap();
    assert_eq!(rx.next().unwrap(), Ok(1));

    tx2.send(2).wait().unwrap();
    assert_eq!(rx.next().unwrap(), Ok(2));
}

#[test]
fn send_recv_threads() {
    let (tx, rx) = mpsc::channel::<i32>(16);
    let mut rx = rx.wait();

    thread::spawn(move|| {
        tx.send(1).wait().unwrap();
    });

    assert_eq!(rx.next().unwrap(), Ok(1));
}

#[test]
fn send_recv_threads_no_capacity() {
    let (mut tx, rx) = mpsc::channel::<i32>(0);
    let mut rx = rx.wait();

    let t = thread::spawn(move|| {
        tx = tx.send(1).wait().unwrap();
        tx = tx.send(2).wait().unwrap();
    });

    thread::sleep(Duration::from_millis(100));
    assert_eq!(rx.next().unwrap(), Ok(1));

    thread::sleep(Duration::from_millis(100));
    assert_eq!(rx.next().unwrap(), Ok(2));

    t.join().unwrap();
}

#[test]
fn recv_close_gets_none() {
    let (tx, mut rx) = mpsc::channel::<i32>(10);

    // Run on a task context
    lazy(move || {
        rx.close();

        assert_eq!(rx.poll(), Ok(Async::Ready(None)));

        drop(tx);

        Ok::<(), ()>(())
    }).wait().unwrap();
}


#[test]
fn tx_close_gets_none() {
    let (_, mut rx) = mpsc::channel::<i32>(10);

    // Run on a task context
    lazy(move || {
        assert_eq!(rx.poll(), Ok(Async::Ready(None)));
        assert_eq!(rx.poll(), Ok(Async::Ready(None)));

        Ok::<(), ()>(())
    }).wait().unwrap();
}

#[test]
fn spawn_sends_items() {
    let core = local_executor::Core::new();
    let stream = unfold(0, |i| Some(ok::<_,u8>((i, i + 1))));
    let rx = mpsc::spawn(stream, &core, 1);
    assert_eq!(core.run(rx.take(4).collect()).unwrap(),
               [0, 1, 2, 3]);
}

#[test]
fn stress_shared_unbounded() {
    const AMT: u32 = 10000;
    const NTHREADS: u32 = 8;
    let (tx, rx) = mpsc::unbounded::<i32>();
    let mut rx = rx.wait();

    let t = thread::spawn(move|| {
        for _ in 0..AMT * NTHREADS {
            assert_eq!(rx.next().unwrap(), Ok(1));
        }

        if rx.next().is_some() {
            panic!();
        }
    });

    for _ in 0..NTHREADS {
        let tx = tx.clone();

        thread::spawn(move|| {
            for _ in 0..AMT {
                mpsc::UnboundedSender::send(&tx, 1).unwrap();
            }
        });
    }

    drop(tx);

    t.join().ok().unwrap();
}

#[test]
fn stress_shared_bounded_hard() {
    const AMT: u32 = 10000;
    const NTHREADS: u32 = 8;
    let (tx, rx) = mpsc::channel::<i32>(0);
    let mut rx = rx.wait();

    let t = thread::spawn(move|| {
        for _ in 0..AMT * NTHREADS {
            assert_eq!(rx.next().unwrap(), Ok(1));
        }

        if rx.next().is_some() {
            panic!();
        }
    });

    for _ in 0..NTHREADS {
        let mut tx = tx.clone();

        thread::spawn(move|| {
            for _ in 0..AMT {
                tx = tx.send(1).wait().unwrap();
            }
        });
    }

    drop(tx);

    t.join().ok().unwrap();
}

#[test]
fn stress_receiver_multi_task_bounded_hard() {
    const AMT: usize = 10_000;
    const NTHREADS: u32 = 2;

    let (mut tx, rx) = mpsc::channel::<usize>(0);
    let rx = Arc::new(Mutex::new(Some(rx)));
    let n = Arc::new(AtomicUsize::new(0));

    let mut th = vec![];

    for _ in 0..NTHREADS {
        let rx = rx.clone();
        let n = n.clone();

        let t = thread::spawn(move || {
            let mut i = 0;

            loop {
                i += 1;
                let mut lock = rx.lock().ok().unwrap();

                match lock.take() {
                    Some(mut rx) => {
                        if i % 5 == 0 {
                            let (item, rest) = rx.into_future().wait().ok().unwrap();

                            if item.is_none() {
                                break;
                            }

                            n.fetch_add(1, Ordering::Relaxed);
                            *lock = Some(rest);
                        } else {
                            // Just poll
                            let n = n.clone();
                            let r = lazy(move || {
                                let r = match rx.poll().unwrap() {
                                    Async::Ready(Some(_)) => {
                                        n.fetch_add(1, Ordering::Relaxed);
                                        *lock = Some(rx);
                                        false
                                    }
                                    Async::Ready(None) => {
                                        true
                                    }
                                    Async::NotReady => {
                                        *lock = Some(rx);
                                        false
                                    }
                                };

                                Ok::<bool, ()>(r)
                            }).wait().unwrap();

                            if r {
                                break;
                            }
                        }
                    }
                    None => break,
                }
            }
        });

        th.push(t);
    }

    for i in 0..AMT {
        tx = tx.send(i).wait().unwrap();
    }

    drop(tx);

    for t in th {
        t.join().unwrap();
    }

    assert_eq!(AMT, n.load(Ordering::Relaxed));
}

/// Stress test that receiver properly receives all the messages
/// after sender dropped.
#[test]
fn stress_drop_sender() {
    fn list() -> Box<Stream<Item=i32, Error=u32>> {
        let (tx, rx) = mpsc::channel(1);
        tx.send(Ok(1))
          .and_then(|tx| tx.send(Ok(2)))
          .and_then(|tx| tx.send(Ok(3)))
          .forget();
        Box::new(rx.then(|r| r.unwrap()))
    }

    for _ in 0..10000 {
        assert_eq!(list().wait().collect::<Result<Vec<_>, _>>(),
        Ok(vec![1, 2, 3]));
    }
}

/// Stress test that after receiver dropped,
/// no messages are lost.
fn stress_close_receiver_iter() {
    let (tx, rx) = mpsc::unbounded();
    let (unwritten_tx, unwritten_rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        for i in 1.. {
            if let Err(_) = mpsc::UnboundedSender::send(&tx, i) {
                unwritten_tx.send(i).expect("unwritten_tx");
                return;
            }
        }
    });

    let mut rx = rx.wait();

    // Read one message to make sure thread effectively started
    assert_eq!(Some(Ok(1)), rx.next());

    rx.get_mut().close();

    for i in 2.. {
        match rx.next() {
            Some(Ok(r)) => assert!(i == r),
            Some(Err(_)) => unreachable!(),
            None => {
                let unwritten = unwritten_rx.recv().expect("unwritten_rx");
                assert_eq!(unwritten, i);
                return;
            }
        }
    }
}

#[test]
fn stress_close_receiver() {
    for _ in 0..10000 {
        stress_close_receiver_iter();
    }
}

fn is_ready<T>(res: &AsyncSink<T>) -> bool {
    match *res {
        AsyncSink::Ready => true,
        _ => false,
    }
}
