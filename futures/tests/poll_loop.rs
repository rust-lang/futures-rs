use futures::iteration::{self, Limit, LoopPolicy, Unlimited};
use futures::prelude::*;
use futures_test::stream::StreamTestExt;
use futures_test::task::{new_count_waker, panic_context};
use std::marker::Unpin;
use std::num::NonZeroU32;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{pin_mut, poll_loop, ready};

// A proof of concept for polling loop instrumentation using the LoopPolicy trait.
#[derive(Debug)]
struct PollOMeter<P> {
    inner: P,
    total_iterations: u64,
    loop_entered: u64,
    loop_yielded: u64,
}

impl<P> PollOMeter<P> {
    fn new(inner: P) -> Self {
        PollOMeter {
            inner,
            total_iterations: 0,
            loop_entered: 0,
            loop_yielded: 0,
        }
    }

    fn average_iterations_per_loop(&self) -> Option<f64> {
        if self.loop_entered != 0 {
            let ratio = self.total_iterations as f64 / self.loop_entered as f64;
            Some(ratio)
        } else {
            None
        }
    }
}

impl<P: LoopPolicy> LoopPolicy for PollOMeter<P> {
    type State = P::State;

    fn enter(&mut self) -> Self::State {
        self.loop_entered += 1;
        self.inner.enter()
    }

    fn yield_check(&mut self, state: &mut Self::State) -> bool {
        self.total_iterations += 1;
        if self.inner.yield_check(state) {
            self.loop_yielded += 1;
            true
        } else {
            false
        }
    }
}

struct Count<S, P = iteration::Limit> {
    stream: S,
    count: u64,
    loop_policy: P,
}

impl<S: Unpin, P> Unpin for Count<S, P> {}

impl<S> Count<S> {
    fn new(stream: S) -> Self {
        Count {
            stream,
            count: 0,
            loop_policy: Limit::new(NonZeroU32::new(64).unwrap()),
        }
    }
}

impl<S, P> Count<S, P> {
    fn with_loop_policy<Q>(self, loop_policy: Q) -> Count<S, Q> {
        Count {
            stream: self.stream,
            count: self.count,
            loop_policy,
        }
    }

    fn loop_policy(&self) -> &P {
        &self.loop_policy
    }

    fn split_borrows(self: Pin<&mut Self>) -> (Pin<&mut S>, &mut P, &mut u64) {
        unsafe {
            let this = self.get_unchecked_mut();
            (
                Pin::new_unchecked(&mut this.stream),
                &mut this.loop_policy,
                &mut this.count,
            )
        }
    }
}

impl<S: Stream, P: LoopPolicy> Future for Count<S, P> {
    type Output = u64;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<u64> {
        let (mut stream, loop_policy, count) = self.split_borrows();
        poll_loop! { loop_policy, cx,
            match ready!(stream.as_mut().poll_next(cx)) {
                Some(_) => {
                    *count += 1;
                }
                None => return Poll::Ready(*count),
            }
        }
    }
}

#[test]
fn unlimited_policy_never_yields() {
    let future = Count::new(stream::repeat(()).take(100000))
        .with_loop_policy(Unlimited);
    pin_mut!(future);
    let mut cx = panic_context();
    assert_eq!(future.as_mut().poll(&mut cx), Poll::Ready(100000));
}

#[test]
fn limit_policy_yields() {
    let future = Count::new(stream::repeat(()).take(11))
        .with_loop_policy(Limit::new(NonZeroU32::new(10).unwrap()));
    pin_mut!(future);
    let (waker, wake_count) = new_count_waker();
    let mut cx = Context::from_waker(&waker);
    assert_eq!(future.as_mut().poll(&mut cx), Poll::Pending);
    assert_eq!(wake_count, 1);
    assert_eq!(future.count, 10);
    assert_eq!(future.as_mut().poll(&mut cx), Poll::Ready(11));
    assert_eq!(wake_count, 1);
}

#[test]
fn custom_mutable_policy_is_feasible() {
    let stream = stream::iter([0, 10, 21].iter().map(|&n| stream::repeat(()).take(n)))
        .interleave_pending()
        .flatten();

    let limit = Limit::new(NonZeroU32::new(20).unwrap());
    let future = Count::new(stream).with_loop_policy(PollOMeter::new(limit));
    pin_mut!(future);

    let (waker, wake_count) = new_count_waker();
    let mut cx = Context::from_waker(&waker);

    assert!(future.loop_policy().average_iterations_per_loop().is_none());
    assert_eq!(future.as_mut().poll(&mut cx), Poll::Pending);
    assert_eq!(wake_count, 1);
    assert_eq!(future.count, 0);
    assert_eq!(future.loop_policy.loop_entered, 1);
    assert_eq!(future.loop_policy.loop_yielded, 0);
    assert_eq!(future.loop_policy.total_iterations, 0);
    assert_eq!(future.as_mut().poll(&mut cx), Poll::Pending);
    assert_eq!(wake_count, 2);
    assert_eq!(future.count, 0);
    assert_eq!(future.loop_policy.loop_entered, 2);
    assert_eq!(future.loop_policy.loop_yielded, 0);
    assert_eq!(future.loop_policy.total_iterations, 0);
    assert_eq!(future.as_mut().poll(&mut cx), Poll::Pending);
    assert_eq!(wake_count, 3);
    assert_eq!(future.count, 10);
    assert_eq!(future.loop_policy.loop_entered, 3);
    assert_eq!(future.loop_policy.loop_yielded, 0);
    assert_eq!(future.loop_policy.total_iterations, 10);
    assert_eq!(future.as_mut().poll(&mut cx), Poll::Pending);
    assert_eq!(wake_count, 4);
    assert_eq!(future.count, 30);
    assert_eq!(future.loop_policy.loop_entered, 4);
    assert_eq!(future.loop_policy.loop_yielded, 1);
    assert_eq!(future.loop_policy.total_iterations, 30);
    assert_eq!(future.loop_policy().average_iterations_per_loop(), Some(7.5));
    assert_eq!(future.as_mut().poll(&mut cx), Poll::Pending);
    assert_eq!(wake_count, 5);
    assert_eq!(future.count, 31);
    assert_eq!(future.loop_policy.loop_entered, 5);
    assert_eq!(future.loop_policy.loop_yielded, 1);
    assert_eq!(future.loop_policy.total_iterations, 31);
    assert_eq!(future.as_mut().poll(&mut cx), Poll::Ready(31));
    assert_eq!(wake_count, 5);
    assert_eq!(future.loop_policy.loop_entered, 6);
    assert_eq!(future.loop_policy.loop_yielded, 1);
    assert_eq!(future.loop_policy.total_iterations, 31);
}
