use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{self, Poll};

#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
crate enum Chain<Fut1, Fut2, Data> {
    First(Fut1, Option<Data>),
    Second(Fut2),
    Empty,
}

impl<Fut1, Fut2, Data> Chain<Fut1, Fut2, Data>
    where Fut1: Future,
          Fut2: Future,
{
    crate fn new(fut1: Fut1, data: Data) -> Chain<Fut1, Fut2, Data> {
        Chain::First(fut1, Some(data))
    }

    crate fn poll<F>(
        self: PinMut<Self>,
        cx: &mut task::Context,
        f: F,
    ) -> Poll<Fut2::Output>
        where F: FnOnce(Fut1::Output, Data) -> Fut2,
    {
        let mut f = Some(f);

        // Safe to call `get_mut_unchecked` because we won't move the futures.
        let this = unsafe { PinMut::get_mut_unchecked(self) };

        loop {
            let (output, data) = match this {
                Chain::First(fut1, data) => {
                    match unsafe { PinMut::new_unchecked(fut1) }.poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(output) => (output, data.take().unwrap()),
                    }
                }
                Chain::Second(fut2) => {
                    return unsafe { PinMut::new_unchecked(fut2) }.poll(cx);
                }
                Chain::Empty => unreachable!()
            };

            *this = Chain::Empty; // Drop fut1
            let fut2 = (f.take().unwrap())(output, data);
            *this = Chain::Second(fut2)
        }
    }
}
