extern crate futures;

use std::fmt;

use futures::{done, Future, Tokens, PollError};
use futures::stream::*;

fn assert_done<S: Stream>(s: &mut S) {
    match s.poll(&Tokens::all()) {
        Some(Ok(None)) => {}
        Some(Ok(Some(_))) => panic!("stream had more elements"),
        Some(Err(PollError::Other(_))) => panic!("stream had an error"),
        Some(Err(PollError::Panicked(_))) => panic!("stream panicked"),
        None => panic!("stream wasn't ready"),
    }
}

fn assert_not_ready<S: Stream>(s: &mut S) {
    match s.poll(&Tokens::all()) {
        Some(Ok(None)) => panic!("stream is at its end"),
        Some(Ok(Some(_))) => panic!("stream had more elements"),
        Some(Err(PollError::Other(_))) => panic!("stream had an error"),
        Some(Err(PollError::Panicked(_))) => panic!("stream panicked"),
        None => {}
    }
}

fn assert_next<S: Stream>(s: &mut S, item: S::Item)
    where S::Item: Eq + fmt::Debug
{
    match s.poll(&Tokens::all()) {
        Some(Ok(None)) => panic!("stream is at its end"),
        Some(Ok(Some(e))) => assert_eq!(e, item),
        Some(Err(PollError::Other(_))) => panic!("stream had an error"),
        Some(Err(PollError::Panicked(_))) => panic!("stream panicked"),
        None => panic!("stream wasn't ready"),
    }
}

#[test]
fn sequence() {
    let (tx, mut rx) = channel();

    assert_not_ready(&mut rx);
    assert_not_ready(&mut rx);

    let amt = 20;
    send(amt, tx).forget();
    for i in (1..amt + 1).rev() {
        assert_next(&mut rx, i);
    }
    assert_done(&mut rx);

    fn send(n: u32, sender: Sender<u32, u32>)
            -> Box<Future<Item=(), Error=()>> {
        if n == 0 {
            return done(Ok(())).boxed()
        }
        sender.send(Ok(n)).map_err(|_| ()).and_then(move |sender| {
            send(n - 1, sender)
        }).boxed()
    }
}

#[test]
fn drop_sender() {
    let (tx, mut rx) = channel::<u32, u32>();
    drop(tx);
    assert_done(&mut rx);
}
