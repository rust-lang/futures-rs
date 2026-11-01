use std::{
    fmt::Debug,
    pin::pin,
    rc::Rc,
    task::{Context, Poll},
};

use futures::{
    FutureExt,
    channel::oneshot,
    executor::block_on,
    future::{Either, Future, TryJoinAll, err, ok, pending, try_join_all},
};
use futures_test::task::{new_count_waker, noop_context};

#[track_caller]
fn assert_done<T>(actual_fut: impl Future<Output = T>, expected: T)
where
    T: PartialEq + Debug,
{
    let output = block_on(pin!(actual_fut));
    assert_eq!(output, expected);
}

#[test]
fn collect_collects() {
    assert_done(try_join_all(vec![ok(1), ok(2)]), Ok::<_, usize>(vec![1, 2]));
    assert_done(try_join_all(vec![ok(1), err(2)]), Err(2));
    assert_done(try_join_all(vec![ok(1)]), Ok::<_, usize>(vec![1]));
    // REVIEW: should this be implemented?
    // assert_done(try_join_all(Vec::<i32>::new()), Ok(vec![]));

    // TODO: needs more tests
}

#[test]
fn try_join_all_iter_lifetime() {
    // In futures-rs version 0.1, this function would fail to typecheck due to an overly
    // conservative type parameterization of `TryJoinAll`.
    fn sizes(bufs: Vec<&[u8]>) -> impl Future<Output = Result<Vec<usize>, ()>> {
        let iter = bufs.into_iter().map(|b| ok::<usize, ()>(b.len()));
        try_join_all(iter)
    }

    assert_done(sizes(vec![&[1, 2, 3], &[], &[0]]), Ok(vec![3_usize, 0, 1]));
}

#[test]
fn try_join_all_from_iter() {
    assert_done(
        vec![ok(1), ok(2)].into_iter().collect::<TryJoinAll<_>>(),
        Ok::<_, usize>(vec![1, 2]),
    )
}

#[test]
fn ready_error_is_not_blocked_by_pending_futures() {
    for len in [2, 30, 31, 64] {
        let inputs = (0..len).map(|index| {
            if index == len - 1 {
                Either::Right(err::<usize, _>(7))
            } else {
                Either::Left(pending())
            }
        });
        assert_eq!(try_join_all(inputs).now_or_never(), Some(Err(7)), "len = {len}");
    }
}

#[test]
fn ready_error_with_unknown_size_hint() {
    let mut inputs = vec![Either::Left(pending()), Either::Right(err::<usize, _>(7))].into_iter();
    let inputs = std::iter::from_fn(move || inputs.next());
    assert_eq!(try_join_all(inputs).now_or_never(), Some(Err(7)));
}

#[test]
fn error_cancels_pending_futures_before_returning() {
    let remaining = Rc::new(());
    let reference = remaining.clone();
    let pending = futures::future::poll_fn(move |_| {
        let _ = &reference;
        Poll::<Result<Rc<()>, u8>>::Pending
    });
    let mut inputs =
        vec![Either::Right(ok(remaining.clone())), Either::Left(pending), Either::Right(err(7))]
            .into_iter();
    let mut joined = pin!(try_join_all(std::iter::from_fn(move || inputs.next())));
    assert_eq!(joined.as_mut().poll(&mut noop_context()), Poll::Ready(Err(7)));
    // Keep joined alive to ensure cancellation happens when returning the error.
    assert_eq!(Rc::strong_count(&remaining), 1);
}

#[test]
fn later_error_wakes_collector() {
    let (sender, receiver) = oneshot::channel::<Result<usize, u8>>();
    let mut inputs =
        vec![Either::Left(pending()), Either::Right(receiver.map(Result::unwrap))].into_iter();
    let mut joined = pin!(try_join_all(std::iter::from_fn(move || inputs.next())));
    let (waker, counter) = new_count_waker();
    let mut cx = Context::from_waker(&waker);
    assert_eq!(joined.as_mut().poll(&mut cx), Poll::Pending);
    let wakes = counter.get();
    sender.send(Err(7)).unwrap();
    assert!(counter.get() > wakes);
    assert_eq!(joined.as_mut().poll(&mut cx), Poll::Ready(Err(7)));
}

#[test]
fn successful_results_keep_input_order() {
    for reverse in [false, true] {
        for len in [2, 31, 64] {
            let (senders, receivers): (Vec<_>, Vec<_>) =
                (0..len).map(|_| oneshot::channel()).unzip();
            let mut joined = pin!(try_join_all(receivers));
            assert_eq!(joined.as_mut().poll(&mut noop_context()), Poll::Pending);
            let mut senders: Vec<_> = senders.into_iter().enumerate().collect();
            if reverse {
                senders.reverse();
            } else {
                senders.rotate_left(1);
            }
            for (index, sender) in senders {
                sender.send(index).unwrap();
                let result = joined.as_mut().poll(&mut noop_context());
                if index == 0 {
                    assert_eq!(result, Poll::Ready(Ok((0..len).collect())));
                } else {
                    assert_eq!(result, Poll::Pending);
                }
            }
        }
    }
}

#[test]
fn empty_iterator_with_unknown_size_hint() {
    let inputs = std::iter::from_fn(|| None::<futures::future::Ready<Result<usize, u8>>>);
    assert_eq!(try_join_all(inputs).now_or_never(), Some(Ok(Vec::new())));
}
