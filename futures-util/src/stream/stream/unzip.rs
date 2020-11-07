use crate::task::AtomicWaker;
use alloc::sync::{Arc, Weak};
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, pinned_drop};
use std::sync::mpsc;

/// SAFETY: safe because only one of two unzipped streams is guaranteed
/// to be accessing underlying stream. This is guaranteed by mpsc. Right
/// stream will access underlying stream only if Sender (or left stream)
/// is dropped in which case try_recv returns disconnected error.
unsafe fn poll_unzipped<S, T1, T2>(
    stream: Pin<&mut Arc<S>>,
    cx: &mut Context<'_>,
) -> Poll<Option<S::Item>>
where
    S: Stream<Item = (T1, T2)>,
{
    stream
        .map_unchecked_mut(|x| &mut *(Arc::as_ptr(x) as *mut S))
        .poll_next(cx)
}

#[pin_project(PinnedDrop)]
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct UnzipLeft<S, T1, T2>
where
    S: Stream<Item = (T1, T2)>,
{
    #[pin]
    stream: Arc<S>,
    right_waker: Weak<AtomicWaker>,
    right_queue: mpsc::Sender<Option<T2>>,
}

impl<S, T1, T2> UnzipLeft<S, T1, T2>
where
    S: Stream<Item = (T1, T2)>,
{
    fn send_to_right(&self, value: Option<T2>) {
        if let Some(right_waker) = self.right_waker.upgrade() {
            // if right_waker.upgrade() succeeds, then right is not
            // dropped so send won't fail.
            let _ = self.right_queue.send(value);
            right_waker.wake();
        }
    }
}

impl<S, T1, T2> FusedStream for UnzipLeft<S, T1, T2>
where
    S: Stream<Item = (T1, T2)> + FusedStream,
{
    fn is_terminated(&self) -> bool {
        self.stream.as_ref().is_terminated()
    }
}

impl<S, T1, T2> Stream for UnzipLeft<S, T1, T2>
where
    S: Stream<Item = (T1, T2)>,
{
    type Item = T1;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().project();

        // SAFETY: for safety details see comment for function: poll_unzipped
        if let Some(value) = ready!(unsafe { poll_unzipped(this.stream, cx) }) {
            self.send_to_right(Some(value.1));
            return Poll::Ready(Some(value.0));
        }
        self.send_to_right(None);
        Poll::Ready(None)
    }
}

#[pinned_drop]
impl<S, T1, T2> PinnedDrop for UnzipLeft<S, T1, T2>
where
    S: Stream<Item = (T1, T2)>,
{
    fn drop(self: Pin<&mut Self>) {
        let this = self.project();
        // wake right stream if it isn't dropped
        if let Some(right_waker) = this.right_waker.upgrade() {
            // drop right_queue sender to cause rx.try_recv to return
            // TryRecvError::Disconnected, so that right knows left is
            // dropped and now it should take over polling the base stream.
            drop(this.right_queue);
            right_waker.wake();
        }
    }
}

#[pin_project]
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct UnzipRight<S, T1, T2>
where
    S: Stream<Item = (T1, T2)>,
{
    #[pin]
    stream: Arc<S>,
    waker: Arc<AtomicWaker>,
    queue: mpsc::Receiver<Option<T2>>,
    is_done: bool,
}

impl<S, T1, T2> FusedStream for UnzipRight<S, T1, T2>
where
    S: FusedStream<Item = (T1, T2)>,
{
    fn is_terminated(&self) -> bool {
        self.is_done
    }
}

impl<S, T1, T2> Stream for UnzipRight<S, T1, T2>
where
    S: Stream<Item = (T1, T2)>,
{
    type Item = T2;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.waker.register(&cx.waker().clone());

        match this.queue.try_recv() {
            Ok(value) => {
                // can't know if more items are in the queue so wake the task
                // again while there are items. Will cause extra wake though.
                cx.waker().clone().wake();
                if value.is_none() {
                    *this.is_done = true;
                }
                Poll::Ready(value)
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                // if left is dropped, it is no longer polling the base stream
                // so right should poll it instead.
                // SAFETY: for safety details see comment for function: poll_unzipped
                if let Some(value) = ready!(unsafe { poll_unzipped(this.stream, cx) }) {
                    return Poll::Ready(Some(value.1));
                }
                *this.is_done = true;
                Poll::Ready(None)
            }
            _ => Poll::Pending,
        }
    }
}

pub fn unzip<S, T1, T2>(stream: S) -> (UnzipLeft<S, T1, T2>, UnzipRight<S, T1, T2>)
where
    S: Stream<Item = (T1, T2)>,
{
    let base_stream = Arc::new(stream);
    let waker = Arc::new(AtomicWaker::new());
    let (tx, rx) = mpsc::channel::<Option<T2>>();

    (
        UnzipLeft {
            stream: base_stream.clone(),
            right_waker: Arc::downgrade(&waker),
            right_queue: tx,
        },
        UnzipRight {
            stream: base_stream.clone(),
            waker: waker,
            queue: rx,
            is_done: false,
        },
    )
}
