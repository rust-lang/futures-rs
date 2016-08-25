use futures::{IntoFuture, Future, Poll};

pub struct SelectAllOk<A>
    where A: Future
{
    inner: Vec<A>,
}

impl<A> Future for SelectAllOk<A>
    where A: Future
{
    type Item = A::Item;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut has_not_ready = false;

        for f in self.inner.iter_mut() {
            match f.poll() {
                Poll::Ok(x) => return Poll::Ok(x),
                Poll::NotReady => {
                    has_not_ready = true;
                }
                _ => {}
            }
        }

        if has_not_ready {
            // at least one of the underlying futures is not finished
            Poll::NotReady
        } else {
            // all the futures are finished, and none of them is Ok
            Poll::Err(())
        }
    }
}

pub fn select_all_ok<I>(iter: I) -> SelectAllOk<<I::Item as IntoFuture>::Future>
    where I: IntoIterator,
          I::Item: IntoFuture
{
    SelectAllOk { inner: iter.into_iter().map(|a| a.into_future()).collect() }
}

#[test]
fn test_ok() {
    use futures::{self, BoxFuture};

    let futs: Vec<BoxFuture<i32, ()>> = vec![futures::finished(3).boxed(),
                                             futures::failed(()).boxed()];
    let res = select_all_ok(futs).wait();
    assert_eq!(res, Ok(3));
}

#[test]
fn test_err() {
    use futures::{self, BoxFuture};

    let futs: Vec<BoxFuture<i32, ()>> = vec![futures::failed(()).boxed(),
                                             futures::failed(()).boxed()];
    let res = select_all_ok(futs).wait();
    assert_eq!(res, Err(()));
}

#[test]
fn test_not_ready() {
    struct Nope;

    impl Future for Nope {
        type Item = ();
        type Error = ();

        fn poll(&mut self) -> Poll<(), ()> {
            Poll::NotReady
        }
    }

    match select_all_ok(vec![Nope]).poll() {
        Poll::NotReady => (),
        _ => panic!(),
    }
}
