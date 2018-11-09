use futures::channel::mpsc;
use futures::executor::{block_on, block_on_stream};
use futures::sink::SinkExt;
use futures::stream::{self, StreamExt};
use std::cell::Cell;
use std::rc::Rc;
use std::thread;

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
