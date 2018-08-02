/// Converts a futures 0.3 `TryFuture`, `TryStream` or `Sink` into a futures 0.1
/// `Future` and vice versa.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Compat<T, Ex> {
    crate inner: T,
    crate executor: Option<Ex>,
}

impl<T, Ex> Compat<T, Ex> {
    /// Returns the inner item.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Creates a new `Compat`.
    crate fn new(inner: T, executor: Option<Ex>) -> Compat<T, Ex> {
        Compat { inner, executor }
    }
}
