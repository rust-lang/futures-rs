use crate::{future::FutureExt, try_future::TryFutureExt};
use std::{
    future::FutureObj,
    task::{Executor, SpawnErrorKind, SpawnObjError},
};
use tokio_executor::{DefaultExecutor, Executor as TokioExecutor};

/// An executor that delegates to `tokio`'s
/// [`DefaultExecutor`][tokio_executor::DefaultExecutor], will panic if used in
/// the context of a task that is not running on `tokio`'s executor.
///
/// # Examples
///
/// ```ignore
/// #![feature(async_await, await_macro, futures_api, pin, use_extern_macros)]
///
/// use std::thread;
/// use std::boxed::PinBox;
///
/// use futures::channel::oneshot;
/// use futures::compat::TokioDefaultExecutor;
/// use futures::future::{FutureExt, TryFutureExt};
/// use futures::spawn;
///
/// let (sender, receiver) = oneshot::channel::<i32>();
///
/// let handle = thread::spawn(move || {
///     futures::executor::block_on(async move {
///         await!(receiver).expect("the sender will send us something")
///     })
/// });
///
/// thread::spawn(move || {
///     let future = async move {
///         spawn!(async move {
///             sender.send(5).expect("the receiver is still waiting");
///         }).expect("can spawn on the executor")
///     };
///
///     let compat_future = PinBox::new(future)
///         .unit_error()
///         .compat(TokioDefaultExecutor);
///
///     tokio::run(compat_future);
/// }).join().unwrap();
///
/// assert_eq!(handle.join().unwrap(), 5);
/// ```
#[derive(Debug, Copy, Clone)]
pub struct TokioDefaultExecutor;

impl Executor for TokioDefaultExecutor {
    fn spawn_obj(
        &mut self,
        task: FutureObj<'static, ()>,
    ) -> Result<(), SpawnObjError> {
        let fut = Box::new(task.unit_error().compat(*self));
        DefaultExecutor::current().spawn(fut).map_err(|err| {
            panic!(
                "tokio failed to spawn and doesn't return the future: {:?}",
                err
            )
        })
    }

    fn status(&self) -> Result<(), SpawnErrorKind> {
        DefaultExecutor::current().status().map_err(|e| {
            if e.is_shutdown() {
                SpawnErrorKind::shutdown()
            } else {
                panic!("tokio executor failed for non-shutdown reason: {:?}", e)
            }
        })
    }
}
