use {Future, Stream};

/// Trait for types which manage a set of `Future`s.
///
/// A `FuturesSet` polls its set of `Future`s and returns produced values
/// through its implementation of `Stream`.
///
/// Two implementations of `FuturesSet` are provided:
/// `FuturesUnordered` and `FuturesOrdered`, which produce items in the order
/// in which they are produced and the order which their corresponding futures
/// were added to the set, respectively.
pub trait FuturesSet<T> : Stream<Item = T::Item, Error = T::Error>
    where T: Future
{
    /// Constructs a new, empty implementation of `FuturesSet`.
    ///
    /// The returned `FuturesSet` does not contain any futures and, in this
    /// state, `poll` will return `Ok(Async::Ready(None))`.
    fn new() -> Self;

    /// Returns the number of futures contained in the set.
    fn len(&self) -> usize;

    /// Push a future into the set.
    ///
    /// This function submits the given future to the set for managing. This
    /// function will not call `poll` on the submitted future. The caller must
    /// ensure that `poll` is called in order to receive task notifications.
    fn push(&mut self, future: T);
}
