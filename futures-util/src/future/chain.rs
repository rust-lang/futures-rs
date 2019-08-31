use core::mem;
use core::pin::Pin;
use core::ptr;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};

#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub(crate) enum Chain<Fut1, Fut2, Data> {
    First(Fut1, Data),
    Second(Fut2),
    Empty,
}

impl<Fut1: Unpin, Fut2: Unpin, Data> Unpin for Chain<Fut1, Fut2, Data> {}

impl<Fut1, Fut2, Data> Chain<Fut1, Fut2, Data> {
    pub(crate)fn is_terminated(&self) -> bool {
        if let Chain::Empty = *self { true } else { false }
    }
}

struct UnsafeEmptyOnDrop<Fut1, Fut2, Data>(*mut Chain<Fut1, Fut2, Data>);

impl<Fut1, Fut2, Data> Drop for UnsafeEmptyOnDrop<Fut1, Fut2, Data> {
    fn drop(&mut self) {
        unsafe {
            ptr::write(self.0, Chain::Empty);
        }
    }
}

impl<Fut1, Fut2, Data> Chain<Fut1, Fut2, Data>
    where Fut1: Future,
          Fut2: Future,
{
    pub(crate) fn new(fut1: Fut1, data: Data) -> Chain<Fut1, Fut2, Data> {
        Chain::First(fut1, data)
    }

    fn take_data(&mut self) -> Data {
        unsafe {
            let auto_empty = UnsafeEmptyOnDrop(self);
            let data = match self {
                Chain::First(fut1, data) => {
                    ptr::drop_in_place(fut1);
                    ptr::read(data)
                }
                _ => unreachable!()
            };
            mem::drop(auto_empty);
            data
        }
    }

    pub(crate) fn poll<F>(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        f: F,
    ) -> Poll<Fut2::Output>
        where F: FnOnce(Fut1::Output, Data) -> Fut2,
    {
        let mut f = Some(f);

        // Safe to call `get_unchecked_mut` because we won't move the futures.
        let this = unsafe { self.get_unchecked_mut() };

        loop {
            let output = match this {
                Chain::First(fut1, _data) => {
                    ready!(unsafe { Pin::new_unchecked(fut1) }.poll(cx))
                }
                Chain::Second(fut2) => {
                    return unsafe { Pin::new_unchecked(fut2) }.poll(cx);
                }
                Chain::Empty => unreachable!()
            };

            let data = Chain::take_data(this); // Drop fut1
            let fut2 = (f.take().unwrap())(output, data);
            *this = Chain::Second(fut2)
        }
    }
}
