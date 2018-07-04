#![feature(pin, arbitrary_self_types, futures_api)]

#[macro_use]
extern crate futures;

use futures::future;
use futures::stream;

mod support;
use self::support::assert::*;

#[test]
fn unfold1() {
    let stream = stream::unfold(0, |state| {
        if state <= 2 {
            support::delayed(future::ready(Some((state * 2, state + 1))))
        } else {
            support::delayed(future::ready(None))
        }
    });

    pin_mut!(stream);

    // Creates the future with the closure
    // Not ready (delayed future)
    assert_stream_pending(stream.reborrow());
    // Future is ready, yields the item
    assert_stream_next(stream.reborrow(), 0);

    // Repeat
    assert_stream_pending(stream.reborrow());
    assert_stream_next(stream.reborrow(), 2);

    assert_stream_pending(stream.reborrow());
    assert_stream_next(stream.reborrow(), 4);

    // No more items
    assert_stream_pending(stream.reborrow());
    assert_stream_done(stream.reborrow());
}
