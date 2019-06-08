use futures_io::{self as io, AsyncBufRead, AsyncRead};
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::{
    pin::Pin,
    task::{Context, Poll},
};

/// Reader for the [`interleave_pending`](super::AsyncReadTestExt::interleave_pending) method.
#[derive(Debug)]
pub struct InterleavePending<R: AsyncRead> {
    reader: R,
    pended: bool,
}

impl<R: AsyncRead + Unpin> Unpin for InterleavePending<R> {}

impl<R: AsyncRead> InterleavePending<R> {
    unsafe_pinned!(reader: R);
    unsafe_unpinned!(pended: bool);

    pub(crate) fn new(reader: R) -> InterleavePending<R> {
        InterleavePending {
            reader,
            pended: false,
        }
    }

    fn project<'a>(self: Pin<&'a mut Self>) -> (Pin<&'a mut R>, &'a mut bool) {
        unsafe {
            let this = self.get_unchecked_mut();
            (Pin::new_unchecked(&mut this.reader), &mut this.pended)
        }
    }
}

impl<R: AsyncRead> AsyncRead for InterleavePending<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let (reader, pended) = self.project();
        if *pended {
            let next = reader.poll_read(cx, buf);
            if next.is_ready() {
                *pended = false;
            }
            next
        } else {
            cx.waker().wake_by_ref();
            *pended = true;
            Poll::Pending
        }
    }
}

impl<R: AsyncBufRead> AsyncBufRead for InterleavePending<R> {
    fn poll_fill_buf<'a>(
        self: Pin<&'a mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<&'a [u8]>> {
        let (reader, pended) = self.project();
        if *pended {
            let next = reader.poll_fill_buf(cx);
            if next.is_ready() {
                *pended = false;
            }
            next
        } else {
            cx.waker().wake_by_ref();
            *pended = true;
            Poll::Pending
        }
    }

    fn consume(self: Pin<&mut Self>, amount: usize) {
        self.reader().consume(amount)
    }
}
