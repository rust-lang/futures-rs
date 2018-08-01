use futures_core::future::Future;
use futures_core::task::Executor;

mod spawn_error;
pub use self::spawn_error::SpawnError;

if_std! {
    mod spawn_with_handle;
    use self::spawn_with_handle::spawn_with_handle;
    pub use self::spawn_with_handle::JoinHandle;
}

impl<Ex: ?Sized> ExecutorExt for Ex where Ex: Executor {}

/// Extension trait for `Executor`
pub trait ExecutorExt: Executor {
    /// Spawns a task that polls the given future with output `()` to
    /// completion.
    ///
    /// This method returns a [`Result`] that contains a [`SpawnError`] if
    /// spawning fails.
    ///
    /// You can use [`spawn_with_handle`](ExecutorExt::spawn_with_handle) if
    /// you want to spawn a future with output other than `()` or if you want
    /// to be able to await its completion.
    ///
    /// Note this method will eventually be replaced with the upcoming
    /// `Executor::spawn` method which will take a `dyn Future` as input.
    /// Technical limitations prevent `Executor::spawn` from being implemented
    /// today. Feel free to use this method in the meantime.
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::executor::ThreadPool;
    /// use futures::task::ExecutorExt;
    ///
    /// let mut executor = ThreadPool::new().unwrap();
    ///
    /// let future = async { /* ... */ };
    /// executor.spawn(future).unwrap();
    /// # });
    /// ```
    #[cfg(feature = "std")]
    fn spawn<Fut>(&mut self, future: Fut) -> Result<(), SpawnError>
    where Fut: Future<Output = ()> + Send + 'static,
    {
        let res = self.spawn_obj(Box::new(future).into());
        res.map_err(|err| SpawnError { kind: err.kind })
    }

    /// Spawns a task that polls the given future to completion and returns a
    /// future that resolves to the spawned future's output.
    ///
    /// This method returns a [`Result`] that contains a [`JoinHandle`], or, if
    /// spawning fails, a [`SpawnError`]. [`JoinHandle`] is a future that
    /// resolves to the output of the spawned future.
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::executor::ThreadPool;
    /// use futures::future;
    /// use futures::task::ExecutorExt;
    ///
    /// let mut executor = ThreadPool::new().unwrap();
    ///
    /// let future = future::ready(1);
    /// let join_handle = executor.spawn_with_handle(future).unwrap();
    /// assert_eq!(await!(join_handle), 1);
    /// # });
    /// ```
    #[cfg(feature = "std")]
    fn spawn_with_handle<Fut>(
        &mut self,
        future: Fut
    ) -> Result<JoinHandle<Fut::Output>, SpawnError>
    where Fut: Future + Send + 'static,
          Fut::Output: Send,
    {
        spawn_with_handle(self, future)
    }
}

