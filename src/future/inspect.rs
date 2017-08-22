use {Future, Poll, Async};

/// Do something with the item of a future, passing it on.
///
/// This is created by the `Future::inspect` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Inspect<A, F> where A: Future {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> Inspect<A, F>
    where A: Future,
{
    Inspect {
        future: future,
        f: Some(f),
    }
}

impl<A, F> Future for Inspect<A, F>
    where A: Future,
          F: FnOnce(&A::Item) -> (),
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self) -> Poll<A::Item, A::Error> {
        match self.future.poll() {
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Ok(Async::Ready(e)) => {
                (self.f.take().expect("cannot poll Inspect twice"))(&e);
                Ok(Async::Ready(e))
            },
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Future;
    use future::{ok, err};

    #[test]
    fn smoke() {
        let mut counter = 0;

        {
            let work = ok::<u32, u32>(40).inspect(|val| { counter += *val; });
            assert_eq!(work.wait(), Ok(40));
        }

        assert_eq!(counter, 40);

        {
            let work = err::<u32, u32>(4).inspect(|val| { counter += *val; });
            assert_eq!(work.wait(), Err(4));
        }

        assert_eq!(counter, 40);
    }
}
