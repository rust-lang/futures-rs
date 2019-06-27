//! The `select` macro.

use proc_macro_hack::proc_macro_hack;

#[doc(hidden)]
#[macro_export]
macro_rules! document_select_macro {
    ($item:item) => {
        /// Polls multiple futures and streams simultaneously, executing the branch
        /// for the future that finishes first. If multiple futures are ready,
        /// one will be pseudo-randomly selected at runtime. Futures passed to
        /// `select!` must be `Unpin` and implement `FusedFuture`.
        /// Futures and streams which are not already fused can be fused using the
        /// `.fuse()` method. Note, though, that fusing a future or stream directly
        /// in the call to `select!` will not be enough to prevent it from being
        /// polled after completion if the `select!` call is in a loop, so when
        /// `select!`ing in a loop, users should take care to `fuse()` outside of
        /// the loop.
        ///
        /// `select!` can select over futures with different output types, but each
        /// branch has to have the same return type.
        ///
        /// This macro is only usable inside of async functions, closures, and blocks.
        /// It is also gated behind the `async-await` feature of this library, which is
        /// _not_ activated by default.
        ///
        /// # Examples
        ///
        /// ```
        /// #![feature(async_await)]
        /// # futures::executor::block_on(async {
        /// use futures::future;
        /// use futures::select;
        /// let mut a = future::ready(4);
        /// let mut b = future::pending::<()>();
        ///
        /// let res = select! {
        ///     a_res = a => a_res + 1,
        ///     _ = b => 0,
        /// };
        /// assert_eq!(res, 5);
        /// # });
        /// ```
        ///
        /// ```
        /// #![feature(async_await)]
        /// # futures::executor::block_on(async {
        /// use futures::future;
        /// use futures::stream::{self, StreamExt};
        /// use futures::select;
        /// let mut st = stream::iter(vec![2]).fuse();
        /// let mut fut = future::pending::<()>();
        ///
        /// select! {
        ///     x = st.next() => assert_eq!(Some(2), x),
        ///     _ = fut => panic!(),
        /// };
        /// # });
        /// ```
        ///
        /// `select` also accepts a `complete` branch and a `default` branch.
        /// `complete` will run if all futures and streams have already been
        /// exhausted. `default` will run if no futures or streams are
        /// immediately ready. `complete` takes priority over `default` in
        /// the case where all futures have completed.
        ///
        /// ```
        /// #![feature(async_await)]
        /// # futures::executor::block_on(async {
        /// use futures::future;
        /// use futures::select;
        /// let mut a_fut = future::ready(4);
        /// let mut b_fut = future::ready(6);
        /// let mut total = 0;
        ///
        /// loop {
        ///     select! {
        ///         a = a_fut => total += a,
        ///         b = b_fut => total += b,
        ///         complete => break,
        ///         default => panic!(), // never runs (futures run first, then complete)
        ///     };
        /// }
        /// assert_eq!(total, 10);
        /// # });
        /// ```
        ///
        /// Note that the futures that have been matched over can still be mutated
        /// from inside the `select!` block's branches. This can be used to implement
        /// more complex behavior such as timer resets or writing into the head of
        /// a stream.
        $item
    }
}

document_select_macro! {
    #[proc_macro_hack(support_nested)]
    pub use futures_select_macro::select;
}
