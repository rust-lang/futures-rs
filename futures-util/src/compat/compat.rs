/// Converts a futures 0.3 `TryFuture` into a futures 0.1 `Future`
/// and vice versa.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Compat<Fut, Ex> {
    crate future: Fut,
    crate executor: Option<Ex>,
}

impl<Fut, Ex> Compat<Fut, Ex> {
    /// Returns the inner future.
    pub fn into_inner(self) -> Fut {
        self.future
    }

    /// Creates a new `Compat`.
    crate fn new(future: Fut, executor: Option<Ex>) -> Compat<Fut, Ex> {
        Compat { future, executor }
    }
}
