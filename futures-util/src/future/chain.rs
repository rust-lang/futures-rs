use core::mem::Pin;

use futures_core::{Future, Poll};
use futures_core::task;

#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub enum Chain<Fut1, Fut2, Data> {
    First(Fut1, Option<Data>),
    Second(Fut2),
}

impl<Fut1, Fut2, Data> Chain<Fut1, Fut2, Data>
    where Fut1: Future,
          Fut2: Future,
{
    pub fn new(fut1: Fut1, data: Data) -> Chain<Fut1, Fut2, Data> {
        Chain::First(fut1, Some(data))
    }

    pub fn poll<F>(mut self: Pin<Self>, cx: &mut task::Context, f: F) -> Poll<Fut2::Output>
        where F: FnOnce(Fut1::Output, Data) -> Fut2,
    {
        let mut f = Some(f);

        loop {
            // safe to `get_mut` here because we don't move out
            let fut2 = match *unsafe { Pin::get_mut(&mut self) } {
                Chain::First(ref mut fut1, ref mut data) => {
                    // safe to create a new `Pin` because `fut1` will never move
                    // before it's dropped.
                    match unsafe { Pin::new_unchecked(fut1) }.poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(t) => {
                            (f.take().unwrap())(t, data.take().unwrap())
                        }
                    }
                }
                Chain::Second(ref mut fut2) => {
                    // safe to create a new `Pin` because `fut2` will never move
                    // before it's dropped; once we're in `Chain::Second` we stay
                    // there forever.
                    return unsafe { Pin::new_unchecked(fut2) }.poll(cx)
                }
            };

            // safe because we're using the `&mut` to do an assignment, not for moving out
            unsafe {
                // note: it's safe to move the `fut2` here because we haven't yet polled it
                *Pin::get_mut(&mut self) = Chain::Second(fut2);
            }
        }
    }
}
