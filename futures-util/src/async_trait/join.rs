#![allow(non_snake_case)]

use core::fmt;
use core::mem;

use futures_core::{Async, Poll, Async};
use futures_core::task;

macro_rules! generate {
    ($(
        $(#[$doc:meta])*
        ($Join:ident, $new:ident, <A, $($B:ident),*>),
    )*) => ($(
        $(#[$doc])*
        #[must_use = "futures do nothing unless polled"]
        pub struct $Join<A, $($B),*>
            where A: Async,
                  $($B: Async<Error=A::Error>),*
        {
            a: MaybeDone<A>,
            $($B: MaybeDone<$B>,)*
        }

        impl<A, $($B),*> fmt::Debug for $Join<A, $($B),*>
            where A: Async + fmt::Debug,
                  A::Item: fmt::Debug,
                  $(
                      $B: Async<Error=A::Error> + fmt::Debug,
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
            where A: Async,
                  $($B: Async<Error=A::Error>),*
        {
            $Join {
                a: MaybeDone::NotYet(a),
                $($B: MaybeDone::NotYet($B)),*
            }
        }

        impl<A, $($B),*> $Join<A, $($B),*>
            where A: Async,
                  $($B: Async<Error=A::Error>),*
        {
            fn erase(&mut self) {
                self.a = MaybeDone::Gone;
                $(self.$B = MaybeDone::Gone;)*
            }
        }

        impl<A, $($B),*> Async for $Join<A, $($B),*>
            where A: Async,
                  $($B: Async<Error=A::Error>),*
        {
            type Item = (A::Item, $($B::Item),*);
            type Error = A::Error;

            fn poll(&mut self, cx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
                let mut all_done = match self.a.poll(cx) {
                    Ok(done) => done,
                    Err(e) => {
                        self.erase();
                        return Err(e)
                    }
                };
                $(
                    all_done = match self.$B.poll(cx) {
                        Ok(done) => all_done && done,
                        Err(e) => {
                            self.erase();
                            return Err(e)
                        }
                    };
                )*

                if all_done {
                    Ok(Async::Ready((self.a.take(), $(self.$B.take()),*)))
                } else {
                    Ok(Async::Pending)
                }
            }
        }

        // Incoherent-- add to futures-core when stable.
        /*
        impl<A, $($B),*> IntoAsync for (A, $($B),*)
            where A: IntoAsync,
        $(
            $B: IntoAsync<Error=A::Error>
        ),*
        {
            type Async = $Join<A::Async, $($B::Async),*>;
            type Item = (A::Item, $($B::Item),*);
            type Error = A::Error;

            fn into_future(self) -> Self::Async {
                match self {
                    (a, $($B),+) => {
                        $new(
                            IntoAsync::into_future(a),
                            $(IntoAsync::into_future($B)),+
                        )
                    }
                }
            }
        }
        */

    )*)
}

generate! {
    /// Async for the `join` combinator, waiting for two futures to
    /// complete.
    ///
    /// This is created by the `Async::join` method.
    (Join, new, <A, B>),

    /// Async for the `join3` combinator, waiting for three futures to
    /// complete.
    ///
    /// This is created by the `Async::join3` method.
    (Join3, new3, <A, B, C>),

    /// Async for the `join4` combinator, waiting for four futures to
    /// complete.
    ///
    /// This is created by the `Async::join4` method.
    (Join4, new4, <A, B, C, D>),

    /// Async for the `join5` combinator, waiting for five futures to
    /// complete.
    ///
    /// This is created by the `Async::join5` method.
    (Join5, new5, <A, B, C, D, E>),
}

#[derive(Debug)]
enum MaybeDone<A: Async> {
    NotYet(A),
    Done(A::Item),
    Gone,
}

impl<A: Async> MaybeDone<A> {
    fn poll(&mut self, cx: &mut task::Context) -> Result<bool, A::Error> {
        let res = match *self {
            MaybeDone::NotYet(ref mut a) => a.poll(cx)?,
            MaybeDone::Done(_) => return Ok(true),
            MaybeDone::Gone => panic!("cannot poll Join twice"),
        };
        match res {
            Async::Ready(res) => {
                *self = MaybeDone::Done(res);
                Ok(true)
            }
            Async::Pending => Ok(false),
        }
    }

    fn take(&mut self) -> A::Item {
        match mem::replace(self, MaybeDone::Gone) {
            MaybeDone::Done(a) => a,
            _ => panic!(),
        }
    }
}
