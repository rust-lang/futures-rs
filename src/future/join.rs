#![allow(non_snake_case)]

use core::fmt;
use core::mem;

use {Future, Poll, IntoFuture, Async};

macro_rules! generate {
    ($(
        $(#[$doc:meta])*
        ($Join:ident, $count: expr, $new:ident, <A, $($B:ident),*>),
    )*) => ($(
        $(#[$doc])*
        #[must_use = "futures do nothing unless polled"]
        pub struct $Join<A, $($B),*>
            where A: Future,
                  $($B: Future<Error=A::Error>),*
        {
            poll_idx: usize,
            a: MaybeDone<A>,
            $($B: MaybeDone<$B>,)*
        }

        impl<A, $($B),*> fmt::Debug for $Join<A, $($B),*>
            where A: Future + fmt::Debug,
                  A::Item: fmt::Debug,
                  $(
                      $B: Future<Error=A::Error> + fmt::Debug,
                      $B::Item: fmt::Debug
                  ),*
        {
            fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt.debug_struct(stringify!($Join))
                    .field("a", &self.a)
                    $(.field(stringify!($B), &self.$B))*
                    .finish()
            }
        }

        pub fn $new<A, $($B),*>(a: A, $($B: $B),*) -> $Join<A, $($B),*>
            where A: Future,
                  $($B: Future<Error=A::Error>),*
        {
            $Join {
                poll_idx: 0,
                a: MaybeDone::NotYet(a),
                $($B: MaybeDone::NotYet($B)),*
            }
        }

        impl<A, $($B),*> $Join<A, $($B),*>
            where A: Future,
                  $($B: Future<Error=A::Error>),*
        {
            fn erase(&mut self) {
                self.a = MaybeDone::Gone;
                $(self.$B = MaybeDone::Gone;)*
            }
        }

        impl<A, $($B),*> Future for $Join<A, $($B),*>
            where A: Future,
                  $($B: Future<Error=A::Error>),*
        {
            type Item = (A::Item, $($B::Item),*);
            type Error = A::Error;

            fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
                let count = $count;

                let mut all_done = true;
                for i in (self.poll_idx..count).chain(0..self.poll_idx) {
                    let mut tracked = 0;
                    if i == 0 {
                        all_done = match self.a.poll() {
                            Ok(done) => all_done && done,
                            Err(e) => {
                                self.erase();
                                return Err(e)
                            }
                        };
                    }

                    $(
                        tracked += 1;
                        if i == tracked {
                            all_done = match self.$B.poll() {
                                Ok(done) => all_done && done,
                                Err(e) => {
                                    self.erase();
                                    return Err(e)
                                }
                            }
                        }
                    )*
                }

                self.poll_idx = (self.poll_idx + 1) % count;

                if all_done {
                    Ok(Async::Ready((self.a.take(), $(self.$B.take()),*)))
                } else {
                    Ok(Async::NotReady)
                }
            }
        }

        impl<A, $($B),*> IntoFuture for (A, $($B),*)
            where A: IntoFuture,
        $(
            $B: IntoFuture<Error=A::Error>
        ),*
        {
            type Future = $Join<A::Future, $($B::Future),*>;
            type Item = (A::Item, $($B::Item),*);
            type Error = A::Error;

            fn into_future(self) -> Self::Future {
                match self {
                    (a, $($B),+) => {
                        $new(
                            IntoFuture::into_future(a),
                            $(IntoFuture::into_future($B)),+
                        )
                    }
                }
            }
        }

    )*)
}

generate! {
    /// Future for the `join` combinator, waiting for two futures to
    /// complete.
    ///
    /// This is created by the `Future::join` method.
    (Join, 2, new, <A, B>),

    /// Future for the `join3` combinator, waiting for three futures to
    /// complete.
    ///
    /// This is created by the `Future::join3` method.
    (Join3, 3, new3, <A, B, C>),

    /// Future for the `join4` combinator, waiting for four futures to
    /// complete.
    ///
    /// This is created by the `Future::join4` method.
    (Join4, 4, new4, <A, B, C, D>),

    /// Future for the `join5` combinator, waiting for five futures to
    /// complete.
    ///
    /// This is created by the `Future::join5` method.
    (Join5, 5, new5, <A, B, C, D, E>),
}

#[derive(Debug)]
enum MaybeDone<A: Future> {
    NotYet(A),
    Done(A::Item),
    Gone,
}

impl<A: Future> MaybeDone<A> {
    fn poll(&mut self) -> Result<bool, A::Error> {
        let res = match *self {
            MaybeDone::NotYet(ref mut a) => a.poll()?,
            MaybeDone::Done(_) => return Ok(true),
            MaybeDone::Gone => panic!("cannot poll Join twice"),
        };
        match res {
            Async::Ready(res) => {
                *self = MaybeDone::Done(res);
                Ok(true)
            }
            Async::NotReady => Ok(false),
        }
    }

    fn take(&mut self) -> A::Item {
        match mem::replace(self, MaybeDone::Gone) {
            MaybeDone::Done(a) => a,
            _ => panic!(),
        }
    }
}
