use core::pin::Pin;

use futures_core::ready;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;

use pin_project_lite::pin_project;

use crate::future::Either;
use crate::stream::stream::FlattenUnordered;
use crate::StreamExt;

delegate_all!(
    /// Stream for the [`try_flatten_unordered`](super::TryStreamExt::try_flatten_unordered) method.
    TryFlattenUnordered<St>(
        FlattenUnordered<TryFlattenSuccessful<St>>
    ): Debug + Sink + Stream + FusedStream + AccessInner[St, (. .)]
        + New[
            |stream: St, limit: impl Into<Option<usize>>|
                TryFlattenSuccessful::new(stream).flatten_unordered(limit)
        ]
    where
        St: TryStream,
        St::Ok: TryStream,
        <St::Ok as TryStream>::Error: From<St::Error>,
        // Needed because either way compiler can't infer types properly...
        St::Ok: Stream<Item = Result<<St::Ok as TryStream>::Ok, <St::Ok as TryStream>::Error>>
);

pin_project! {
    /// Flattens successful streams from the given stream, bubbling up the errors.
    /// This's a wrapper for `TryFlattenUnordered` to reuse `FlattenUnordered` logic over `TryStream`.
    #[derive(Debug)]
    pub struct TryFlattenSuccessful<St> {
        #[pin]
        stream: St,
    }
}

impl<St> TryFlattenSuccessful<St> {
    fn new(stream: St) -> Self {
        Self { stream }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St> FusedStream for TryFlattenSuccessful<St>
where
    St: TryStream + FusedStream,
    St::Ok: TryStream,
    <St::Ok as TryStream>::Error: From<St::Error>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

/// Emits one item immediately, then stream will be terminated.
#[derive(Debug, Clone)]
pub struct One<T>(Option<T>);

impl<T> Unpin for One<T> {}

impl<T> Stream for One<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.0.take())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.as_ref().map_or((0, Some(0)), |_| (1, Some(1)))
    }
}

type OneResult<St> = One<
    Result<<<St as TryStream>::Ok as TryStream>::Ok, <<St as TryStream>::Ok as TryStream>::Error>,
>;

impl<St> Stream for TryFlattenSuccessful<St>
where
    St: TryStream,
    St::Ok: TryStream,
    <St::Ok as TryStream>::Error: From<St::Error>,
{
    // Item is either an inner stream or a stream containing a single error.
    // This will allow using `Either`'s `Stream` implementation as both branches are actually streams of `Result`'s.
    type Item = Either<St::Ok, OneResult<St>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let item = ready!(self.project().stream.try_poll_next(cx));

        let out = item.map(|res| match res {
            // Emit inner stream as is
            Ok(stream) => Either::Left(stream),
            // Wrap an error into stream wrapper containing one item
            err @ Err(_) => {
                let res = err.map(|_: St::Ok| unreachable!()).map_err(Into::into);

                Either::Right(One(Some(res)))
            }
        });

        Poll::Ready(out)
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for TryFlattenSuccessful<S>
where
    S: TryStream + Sink<Item>,
    S::Ok: TryStream,
    <S::Ok as TryStream>::Error: From<<S as TryStream>::Error>,
{
    type Error = <S as Sink<Item>>::Error;

    delegate_sink!(stream, Item);
}
