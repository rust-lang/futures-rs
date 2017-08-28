use {Async, Never, Stream};
use never::InfallibleResultExt;

/// Stream that can not fail.
pub trait InfallibleStream: Stream {
    /// Poll a stream that can not fail.
    ///
    /// Works similar to `poll`, except that it returns an `Async` value directly
    /// rather than `Poll`.
    fn poll_infallible(&mut self) -> Async<Option<Self::Item>>;
}

impl<S: Stream<Error=Never>> InfallibleStream for S {
    fn poll_infallible(&mut self) -> Async<Option<Self::Item>> {
        self.poll().infallible()
    }
}
