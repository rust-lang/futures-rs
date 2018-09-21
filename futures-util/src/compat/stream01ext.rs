use super::Compat;
use futures::Stream as Stream01;

impl<St: Stream01> Stream01CompatExt for St {}

/// Extension trait for futures 0.1 [`Stream`](futures::stream::Stream)
pub trait Stream01CompatExt: Stream01 {
    /// Converts a futures 0.1
    /// [`Stream<Item = T, Error = E>`](futures::stream::Stream)
    /// into a futures 0.3
    /// [`Stream<Item = Result<T, E>>`](futures_core::stream::Stream).
    fn compat(self) -> Compat<Self, ()> where Self: Sized {
        Compat::new(self, None)
    }
}
