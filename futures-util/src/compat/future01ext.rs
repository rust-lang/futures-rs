use super::Compat;
use futures::Future as Future01;

impl<Fut: Future01> Future01CompatExt for Fut {}

/// Extension trait for futures 0.1 Futures.
pub trait Future01CompatExt: Future01 {
    /// Converts a futures 0.1 `Future<Item = T, Error = E>` into a
    /// futures 0.3 `Future<Output = Result<T, E>>`.
    fn compat(self) -> Compat<Self, ()> where Self: Sized {
        Compat {
            inner: self,
            executor: None,
        }
    }
}


