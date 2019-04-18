use core::pin::Pin;
use core::task::{Context, Poll};
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_sink::Sink;

/// Combines two different futures, streams, or sinks having the same associated types into a single
/// type.
#[derive(Debug, Clone)]
pub enum Either<A, B> {
    /// First branch of the type
    Left(A),
    /// Second branch of the type
    Right(B),
}

impl<A, B, T> Either<(T, A), (T, B)> {
    /// Factor out a homogeneous type from an either of pairs.
    ///
    /// Here, the homogeneous type is the first element of the pairs.
    pub fn factor_first(self) -> (T, Either<A, B>) {
        match self {
            Either::Left((x, a)) => (x, Either::Left(a)),
            Either::Right((x, b)) => (x, Either::Right(b)),
        }
    }
}

impl<A, B, T> Either<(A, T), (B, T)> {
    /// Factor out a homogeneous type from an either of pairs.
    ///
    /// Here, the homogeneous type is the second element of the pairs.
    pub fn factor_second(self) -> (Either<A, B>, T) {
        match self {
            Either::Left((a, x)) => (Either::Left(a), x),
            Either::Right((b, x)) => (Either::Right(b), x),
        }
    }
}

impl<T> Either<T, T> {
    /// Extract the value of an either over two equivalent types.
    pub fn into_inner(self) -> T {
        match self {
            Either::Left(x) => x,
            Either::Right(x) => x,
        }
    }
}

impl<A, B> Future for Either<A, B>
where
    A: Future,
    B: Future<Output = A::Output>,
{
    type Output = A::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<A::Output> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(a) => Pin::new_unchecked(a).poll(cx),
                Either::Right(b) => Pin::new_unchecked(b).poll(cx),
            }
        }
    }
}

impl<A, B> Stream for Either<A, B>
where
    A: Stream,
    B: Stream<Item = A::Item>,
{
    type Item = A::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<A::Item>> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(a) => Pin::new_unchecked(a).poll_next(cx),
                Either::Right(b) => Pin::new_unchecked(b).poll_next(cx),
            }
        }
    }
}

impl<A, B, Item> Sink<Item> for Either<A, B>
where
    A: Sink<Item>,
    B: Sink<Item, SinkError = A::SinkError>,
{
    type SinkError = A::SinkError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::SinkError>> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(x) => Pin::new_unchecked(x).poll_ready(cx),
                Either::Right(x) => Pin::new_unchecked(x).poll_ready(cx),
            }
        }
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::SinkError> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(x) => Pin::new_unchecked(x).start_send(item),
                Either::Right(x) => Pin::new_unchecked(x).start_send(item),
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::SinkError>> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(x) => Pin::new_unchecked(x).poll_flush(cx),
                Either::Right(x) => Pin::new_unchecked(x).poll_flush(cx),
            }
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::SinkError>> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(x) => Pin::new_unchecked(x).poll_close(cx),
                Either::Right(x) => Pin::new_unchecked(x).poll_close(cx),
            }
        }
    }
}
