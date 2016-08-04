use {Task, IntoFuture, Poll};
use stream::{Stream, Fuse};
use util::Collapsed;

/// An adaptor for a stream of futures to execute the futures concurrently, if
/// possible.
///
/// This adaptor will buffer up a list of pending futures, and then return their
/// results in the order that they're finished. This is created by the
/// `Stream::buffered` method.
pub struct Buffered<S>
    where S: Stream,
          S::Item: IntoFuture,
{
    stream: Fuse<S>,
    futures: Vec<Option<Collapsed<<S::Item as IntoFuture>::Future>>>,
}

pub fn new<S>(s: S, amt: usize) -> Buffered<S>
    where S: Stream,
          S::Item: IntoFuture<Error=<S as Stream>::Error>,
{
    Buffered {
        stream: super::fuse::new(s),
        futures: (0..amt).map(|_| None).collect(),
    }
}

impl<S> Stream for Buffered<S>
    where S: Stream,
          S::Item: IntoFuture<Error=<S as Stream>::Error>,
{
    type Item = <S::Item as IntoFuture>::Item;
    type Error = <S as Stream>::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Self::Item>, Self::Error> {
        let mut any_some = false;
        for f in self.futures.iter_mut() {
            // First, if this slot is empty, try to fill it in. If we fill it in
            // we're careful to use TOKENS_ALL for the next poll() below.
            if f.is_none() {
                match self.stream.poll(task) {
                    Poll::Ok(Some(e)) => {
                        *f = Some(Collapsed::Start(e.into_future()));
                    }
                    Poll::Err(e) => return Poll::Err(e),
                    Poll::Ok(None) |
                    Poll::NotReady => continue,
                }
            }

            // If we're here then our slot is full, so we unwrap it and poll it.
            let ret = {
                let future = f.as_mut().unwrap();
                match future.poll(task) {
                    Poll::Ok(e) => Poll::Ok(Some(e)),
                    Poll::Err(e) => Poll::Err(e),

                    // TODO: should this happen here or elsewhere?
                    Poll::NotReady => {
                        future.collapse();
                        any_some = true;
                        continue
                    }
                }
            };

            // Ok, that future is done, so we chuck it out and return its value.
            // Next time we're poll()'d it'll get filled in again.
            *f = None;
            return ret
        }

        if any_some || !self.stream.is_done() {
            Poll::NotReady
        } else {
            Poll::Ok(None)
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        let mut any_none = false;
        // Primarily we're interested in all our pending futures, so schedule a
        // callback on all of them.
        for f in self.futures.iter_mut() {
            match *f {
                Some(ref mut f) => f.schedule(task),
                None => any_none = true,
            }
        }

        // If any slot was None, then we're also interested in the stream, but
        // if all slots were taken we're not actually interested in the stream.
        if any_none {
            self.stream.schedule(task);
        }
    }
}

