/// Spawns a task onto the context's executor that polls the given future with
/// output `()` to completion.
///
/// This macro returns a [`Result`] that contains a
/// [`SpawnError`](crate::task::SpawnError) if spawning fails.
///
/// You can use [`spawn_with_handle!`] if you want to spawn a future
/// with output other than `()` or if you want to be able to await its
/// completion.
///
/// ```
/// #![feature(async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::spawn;
///
/// let future = async { /* ... */ };
/// spawn!(future).unwrap();
/// # });
/// ```
#[macro_export]
macro_rules! spawn {
    ($future:expr) => {
        await!($crate::future::lazy(|cx| {
            $crate::task::SpawnExt::spawn(cx.spawner(), $future)
        }))
    }
}

/// Spawns a task onto the context's executor that polls the given future to
/// completion and returns a future that resolves to the spawned future's
/// output.
///
/// This macro returns a [`Result`] that contains a
/// [`JoinHandle`](crate::task::JoinHandle), or, if spawning fails, a
/// [`SpawnError`](crate::task::SpawnError).
/// [`JoinHandle`](crate::task::JoinHandle) is a future that resolves
/// to the output of the spawned future
///
/// # Examples
///
/// ```
/// #![feature(async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::{future, spawn_with_handle};
///
/// let future = future::ready(1);
/// let join_handle = spawn_with_handle!(future).unwrap();
/// assert_eq!(await!(join_handle), 1);
/// # });
/// ```
#[macro_export]
macro_rules! spawn_with_handle {
    ($future:expr) => {
        await!($crate::future::lazy(|cx| {
            $crate::task::SpawnExt::spawn_with_handle(cx.spawner(), $future)
        }))
    }
}
