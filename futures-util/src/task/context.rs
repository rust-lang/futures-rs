use futures_core::task;

if_std! {
    use futures_core::future::{Future, FutureObj};
    use std::boxed::Box;
}

/// Extension trait for `Context`, adding methods that require allocation.
pub trait ContextExt {
    /// Spawn a future onto the default executor.
    ///
    /// # Panics
    ///
    /// This method will panic if the default executor is unable to spawn.
    ///
    /// To handle executor errors, use `Context::executor()` on instead.
    #[cfg(feature = "std")]
    fn spawn<Fut>(&mut self, future: Fut)
        where Fut: Future<Output = ()> + 'static + Send;
}

impl<'a> ContextExt for task::Context<'a> {
    #[cfg(feature = "std")]
    fn spawn<Fut>(&mut self, future: Fut)
        where Fut: Future<Output = ()> + 'static + Send
    {
        self.executor().spawn_obj(FutureObj::new(Box::new(future))).unwrap()
    }
}
