#![cfg(not(miri))] // https://github.com/rust-lang/miri/issues/1038

use futures::{
    stream::{self, StreamExt, TryStreamExt},
    task::Poll,
};
use futures_executor::block_on;
use futures_test::task::noop_context;

#[test]
fn try_filter_map_after_err() {
    let cx = &mut noop_context();
    let mut s = stream::iter(1..=3)
        .map(Ok)
        .try_filter_map(|v| async move { Err::<Option<()>, _>(v) })
        .filter_map(|r| async move { r.ok() })
        .boxed();
    assert_eq!(Poll::Ready(None), s.poll_next_unpin(cx));
}

#[test]
fn try_skip_while_after_err() {
    let cx = &mut noop_context();
    let mut s = stream::iter(1..=3)
        .map(Ok)
        .try_skip_while(|_| async move { Err::<_, ()>(()) })
        .filter_map(|r| async move { r.ok() })
        .boxed();
    assert_eq!(Poll::Ready(None), s.poll_next_unpin(cx));
}

#[test]
fn try_take_while_after_err() {
    let cx = &mut noop_context();
    let mut s = stream::iter(1..=3)
        .map(Ok)
        .try_take_while(|_| async move { Err::<_, ()>(()) })
        .filter_map(|r| async move { r.ok() })
        .boxed();
    assert_eq!(Poll::Ready(None), s.poll_next_unpin(cx));
}

#[test]
fn try_flatten_unordered() {
    let s = stream::iter(1..5)
        .map(|val: u32| {
            if val % 2 == 0 {
                Ok(stream::unfold((val, 1), |(val, pow)| async move {
                    Some((val.pow(pow), (val, pow + 1)))
                })
                .take(3)
                .map(move |val| if val % 2 == 0 { Ok(val) } else { Err(val) }))
            } else {
                Err(val)
            }
        })
        .try_flatten_unordered(None);

    block_on(async move {
        assert_eq!(
            vec![Err(1), Ok(2), Err(3), Ok(4), Ok(4), Ok(16), Ok(8), Ok(64)],
            s.collect::<Vec<_>>().await
        )
    })
}
