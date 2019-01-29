use futures::channel::mpsc;
use futures::executor::block_on;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std::thread;

#[test]
fn smoke() {
    let (mut sender, receiver) = mpsc::channel(1);

    let t = thread::spawn(move || {
        while let Ok(()) = block_on(sender.send(42)) {}
    });

    // `receiver` needs to be dropped for `sender` to stop sending and therefore before the join.
    drop(block_on(receiver.take(3).for_each(|_| futures::future::ready(()))));

    t.join().unwrap()
}

#[test]
fn multiple_senders_disconnect() {
    {
        let (mut tx1, mut rx) = mpsc::channel(1);
        let (tx2, mut tx3, mut tx4) = (tx1.clone(), tx1.clone(), tx1.clone());

        // disconnect, dropping and Sink::poll_close should all close this sender but leave the
        // channel open for other senders
        tx1.disconnect();
        drop(tx2);
        block_on(tx3.close()).unwrap();

        assert!(tx1.is_closed());
        assert!(tx3.is_closed());
        assert!(!tx4.is_closed());

        block_on(tx4.send(5)).unwrap();
        assert_eq!(block_on(rx.next()), Some(5));

        // dropping the final sender will close the channel
        drop(tx4);
        assert_eq!(block_on(rx.next()), None);
    }

    {
        let (mut tx1, mut rx) = mpsc::unbounded();
        let (tx2, mut tx3, mut tx4) = (tx1.clone(), tx1.clone(), tx1.clone());

        // disconnect, dropping and Sink::poll_close should all close this sender but leave the
        // channel open for other senders
        tx1.disconnect();
        drop(tx2);
        block_on(tx3.close()).unwrap();

        assert!(tx1.is_closed());
        assert!(tx3.is_closed());
        assert!(!tx4.is_closed());

        block_on(tx4.send(5)).unwrap();
        assert_eq!(block_on(rx.next()), Some(5));

        // dropping the final sender will close the channel
        drop(tx4);
        assert_eq!(block_on(rx.next()), None);
    }
}

#[test]
fn multiple_senders_close_channel() {
    {
        let (mut tx1, mut rx) = mpsc::channel(1);
        let mut tx2 = tx1.clone();

        // close_channel should shut down the whole channel
        tx1.close_channel();

        assert!(tx1.is_closed());
        assert!(tx2.is_closed());

        let err = block_on(tx2.send(5)).unwrap_err();
        assert!(err.is_disconnected());

        assert_eq!(block_on(rx.next()), None);
    }

    {
        let (tx1, mut rx) = mpsc::unbounded();
        let mut tx2 = tx1.clone();

        // close_channel should shut down the whole channel
        tx1.close_channel();

        assert!(tx1.is_closed());
        assert!(tx2.is_closed());

        let err = block_on(tx2.send(5)).unwrap_err();
        assert!(err.is_disconnected());

        assert_eq!(block_on(rx.next()), None);
    }
}
