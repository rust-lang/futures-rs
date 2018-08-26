/// Converts a futures 0.3 [`TryFuture`](futures_core::future::TryFuture),
/// [`TryStream`](futures_core::stream::TryStream) or
/// [`Sink`](futures_sink::Sink) into a futures 0.1
/// [`Future`](futures01::future::Future),
/// [`Stream`](futures01::stream::Stream) or
/// [`Sink`](futures01::sink::Sink) and vice versa.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Compat<T, Sp> {
    crate inner: T,
    crate spawn: Option<Sp>,
}

impl<T, Sp> Compat<T, Sp> {
    /// Returns the inner item.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Creates a new [`Compat`].
    crate fn new(inner: T, spawn: Option<Sp>) -> Compat<T, Sp> {
        Compat { inner, spawn }
    }
}
