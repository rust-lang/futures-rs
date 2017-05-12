extern crate futures;

use futures::{Future, Stream};
use futures::Async::*;
use futures::future::{self, ReadyQueue};
use futures::sync::oneshot;

#[test]
fn basic_usage() {
    future::lazy(move || {
        let mut queue = ReadyQueue::new();
        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        let (tx3, rx3) = oneshot::channel();

        queue.push(rx1);
        queue.push(rx2);
        queue.push(rx3);

        assert!(!queue.poll().unwrap().is_ready());

        tx2.send("hello").unwrap();

        assert_eq!(Ready(Some("hello")), queue.poll().unwrap());
        assert!(!queue.poll().unwrap().is_ready());

        tx1.send("world").unwrap();
        tx3.send("world2").unwrap();

        assert_eq!(Ready(Some("world")), queue.poll().unwrap());
        assert_eq!(Ready(Some("world2")), queue.poll().unwrap());
        assert!(!queue.poll().unwrap().is_ready());

        Ok::<_, ()>(())
    }).wait().unwrap();
}

#[test]
fn dropping_ready_queue() {
    future::lazy(move || {
        let mut queue = ReadyQueue::new();
        let (mut tx, rx) = oneshot::channel::<()>();

        queue.push(rx);

        assert!(!tx.poll_cancel().unwrap().is_ready());
        drop(queue);
        assert!(tx.poll_cancel().unwrap().is_ready());

        Ok::<_, ()>(())
    }).wait().unwrap();
}

#[test]
fn stress() {
}
