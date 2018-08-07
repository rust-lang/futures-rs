use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which executes a unit closure over each item on a
/// stream.
///
/// This structure is returned by the `Stream::for_each` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct ForEach<St, Fut, F> {
    stream: St,
    f: F,
    future: Option<Fut>,
}

impl<St, Fut, F> Unpin for ForEach<St, Fut, F>
where St: Stream + Unpin,
      F: FnMut(St::Item) -> Fut,
      Fut: Future<Output = ()> + Unpin,
{}

impl<St, Fut, F> ForEach<St, Fut, F>
where St: Stream,
      F: FnMut(St::Item) -> Fut,
      Fut: Future<Output = ()>,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(future: Option<Fut>);

    pub(super) fn new(stream: St, f: F) -> ForEach<St, Fut, F> {
        ForEach {
            stream,
            f,
            future: None,
        }
    }
}

impl<St, Fut, F> Future for ForEach<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future<Output = ()>,
{
    type Output = ();

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<()> {
        loop {
            if let Some(future) = self.future().as_pin_mut() {
                ready!(future.poll(cx));
            }
            PinMut::set(self.future(), None);

            match ready!(self.stream().poll_next(cx)) {
                Some(e) => {
                    let future = (self.f())(e);
                    PinMut::set(self.future(), Some(future));
                }
                None => {
                    return Poll::Ready(());
                }
            }
        }
    }
}
