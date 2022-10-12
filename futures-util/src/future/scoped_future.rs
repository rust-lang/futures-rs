use crate::future::future::FutureExt;
use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};

#[cfg(feature = "alloc")]
use futures_core::future::{BoxFuture, LocalBoxFuture};

/// A [`Future`] wrapper that imposes a lower limit on the future's lifetime's duration.
/// This is especially useful in combination with higher-tranked bounds when an lifetime
/// bound is needed for the higher-ranked lifetime and a future is used in the bound.
///
/// # Example
/// ```
/// use futures::future::{ScopedBoxFuture, ScopedFutureExt};
///
/// pub struct Db {
///     count: u8,
/// }
///
/// impl Db {
///     async fn transaction<'a, T: 'a, E: 'a, F: 'a>(&mut self, callback: F) -> Result<T, E>
///     where
///         F: for<'b> FnOnce(&'b mut Self) -> ScopedBoxFuture<'a, 'b, Result<T, E>> + Send,
///     {
///         callback(self).await
///     }
/// }
///
/// pub async fn test_transaction<'a, 'b>(
///     db: &mut Db,
///     ok: &'a str,
///     err: &'b str,
///     is_ok: bool,
/// ) -> Result<&'a str, &'b str> {
///     db.transaction(|db| async move {
///         db.count += 1;
///         if is_ok {
///             Ok(ok)
///         } else {
///             Err(err)
///         }
///     }.scope_boxed()).await?;
///
///     // note that `async` is used instead of `async move`
///     // since the callback parameter is unused
///     db.transaction(|_| async {
///         if is_ok {
///             Ok(ok)
///         } else {
///             Err(err)
///         }
///     }.scope_boxed()).await
/// }
/// ```
#[derive(Clone, Debug)]
pub struct ScopedFuture<'lower_bound, 'a, Fut> {
    future: Fut,
    scope: PhantomData<&'a &'lower_bound ()>,
}

/// A boxed future whose lifetime is lower bounded.
#[cfg(feature = "alloc")]
pub type ScopedBoxFuture<'lower_bound, 'a, T> = ScopedFuture<'lower_bound, 'a, BoxFuture<'a, T>>;

/// A non-Send boxed future whose lifetime is lower bounded.
#[cfg(feature = "alloc")]
pub type ScopedLocalBoxFuture<'lower_bound, 'a, T> =
    ScopedFuture<'lower_bound, 'a, LocalBoxFuture<'a, T>>;

/// An extension trait for `Future`s that provides methods for encoding lifetime information of captures.
pub trait ScopedFutureExt: Sized {
    /// Encodes the lifetimes of this `Future`'s captures.
    fn scoped<'lower_bound, 'a>(self) -> ScopedFuture<'lower_bound, 'a, Self>;

    /// Boxes this `Future` and encodes the lifetimes of its captures.
    #[cfg(feature = "std")]
    fn scope_boxed<'lower_bound, 'a>(
        self,
    ) -> ScopedBoxFuture<'lower_bound, 'a, <Self as Future>::Output>
    where
        Self: Send + Future + 'a;

    /// Boxes this non-Send `Future` and encodes the lifetimes of its captures.
    #[cfg(feature = "std")]
    fn scope_boxed_local<'lower_bound, 'a>(
        self,
    ) -> ScopedLocalBoxFuture<'lower_bound, 'a, <Self as Future>::Output>
    where
        Self: Future + 'a;
}

impl<Fut> ScopedFuture<'_, '_, Fut> {
    pin_utils::unsafe_pinned!(future: Fut);
}

impl<Fut: Future> Future for ScopedFuture<'_, '_, Fut> {
    type Output = Fut::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.future().poll(cx)
    }
}

impl<Fut: Future> ScopedFutureExt for Fut {
    fn scoped<'lower_bound, 'a>(self) -> ScopedFuture<'lower_bound, 'a, Self> {
        ScopedFuture { future: self, scope: PhantomData }
    }

    #[cfg(feature = "alloc")]
    fn scope_boxed<'lower_bound, 'a>(
        self,
    ) -> ScopedFuture<'lower_bound, 'a, BoxFuture<'a, <Self as Future>::Output>>
    where
        Self: Send + Future + 'a,
    {
        ScopedFuture { future: self.boxed(), scope: PhantomData }
    }

    #[cfg(feature = "alloc")]
    fn scope_boxed_local<'lower_bound, 'a>(
        self,
    ) -> ScopedFuture<'lower_bound, 'a, LocalBoxFuture<'a, <Self as Future>::Output>>
    where
        Self: Future + 'a,
    {
        ScopedFuture { future: self.boxed_local(), scope: PhantomData }
    }
}
