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
    TryFlattenUnordered<St, I, E>(
        FlattenUnordered<ResultToEither<St, I, E>>
    ): Debug + Sink + Stream + FusedStream + AccessInner[St, (. .)]
        + New[
            |stream: St, limit: impl Into<Option<usize>>|
                ResultToEither::new(stream).flatten_unordered(limit)
        ]
    where
        St: TryStream,
        St::Ok: Stream<Item = Result<I, E>>,
        St::Ok: Unpin,
        E: From<St::Error>
);

pin_project! {
    /// Emits either successful streams or single-item streams containing the underlying errors.
    /// This's a wrapper for `FlattenUnordered` to reuse its logic over `TryStream`.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct ResultToEither<St, I, E>
        where
            St: TryStream,
            St::Ok: Stream<Item = Result<I, E>>,
            St::Ok: Unpin,
            E: From<St::Error>
        {
            #[pin]
            stream: St,
        }
}

impl<St, I, E> ResultToEither<St, I, E>
where
    St: TryStream,
    St::Ok: Stream<Item = Result<I, E>> + Unpin,
    E: From<St::Error>,
{
    fn new(stream: St) -> Self {
        Self { stream }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, I, E> FusedStream for ResultToEither<St, I, E>
where
    St: TryStream + FusedStream,
    St::Ok: Stream<Item = Result<I, E>> + Unpin,
    E: From<St::Error>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

/// Emits single item immediately, then stream will be terminated.
#[derive(Debug, Clone)]
pub struct Single<T>(Option<T>);

impl<T> Unpin for Single<T> {}

impl<T> Stream for Single<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.0.take())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.as_ref().map_or((0, Some(0)), |_| (1, Some(1)))
    }
}

type SingleResult<St> = Single<
    Result<<<St as TryStream>::Ok as TryStream>::Ok, <<St as TryStream>::Ok as TryStream>::Error>,
>;

impl<St, I, E> Stream for ResultToEither<St, I, E>
where
    St: TryStream,
    St::Ok: Stream<Item = Result<I, E>> + Unpin,
    E: From<St::Error>,
{
    // Item is either an inner stream or a stream containing a single error.
    // This will allow using `Either`'s `Stream` implementation as both branches are actually streams of `Result`'s.
    type Item = Either<St::Ok, SingleResult<St>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let item = ready!(self.project().stream.try_poll_next(cx));

        let out = item.map(|res| match res {
            // Emit successful inner stream as is
            Ok(stream) => Either::Left(stream),
            // Wrap an error into a stream containing a single item
            err @ Err(_) => {
                let res = err.map(|_: St::Ok| unreachable!()).map_err(Into::into);

                Either::Right(Single(Some(res)))
            }
        });

        Poll::Ready(out)
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<St, I, E, Item> Sink<Item> for ResultToEither<St, I, E>
where
    St: TryStream + Sink<Item>,
    St::Ok: Stream<Item = Result<I, E>> + Unpin,
    E: From<<St as TryStream>::Error>,
{
    type Error = <St as Sink<Item>>::Error;

    delegate_sink!(stream, Item);
}
