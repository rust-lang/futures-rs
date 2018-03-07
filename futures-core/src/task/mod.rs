//! Task notification.

use core::fmt;

if_std! {
    use core::marker::PhantomData;
    use never::Never;
    use executor::Executor;
    type Exec<'a> = &'a mut Executor;
}

if_not_std! {
    type Exec<'a> = ();
}

mod atomic_waker;
pub use self::atomic_waker::AtomicWaker;

/// Information about the currently-running task.
///
/// Contexts are always tied to the stack, since they are set up specifically
/// when performing a single `poll` step on a task.
pub struct Context<'a> {
    waker: &'a Waker,
    map: &'a mut LocalMap,
    executor: Exec<'a>,
}

impl<'a> Context<'a> {
    /// Get the [`Waker`](::task::Waker) associated with the current task.
    ///
    /// The waker can subsequently be used to wake up the task when some
    /// event of interest has happened.
    pub fn waker(&self) -> &Waker {
        self.waker
    }

    /// Produce a context like the current one, but using the given waker
    /// instead.
    ///
    /// This advanced method is primarily used when building "internal
    /// schedulers" within a task, where you want to provide some customized
    /// wakeup logic.
    pub fn with_waker<'b>(&'b mut self, waker: &'b Waker) -> Context<'b> {
        Context { map: self.map, executor: self.executor, waker }
    }

    /// Produce a context like the current one, but using the given task locals
    /// instead.
    ///
    /// This advanced method is primarily used when building "internal
    /// schedulers" within a task.
    pub fn with_locals<'b>(&'b mut self, map: &'b mut LocalMap)
        -> Context<'b>
    {
        Context { map, waker: self.waker, executor: self.executor }
    }
}

if_std! {
    use std::boxed::Box;
    use Future;

    impl<'a> Context<'a> {
        /// Create a new task context.
        ///
        /// Task contexts are always equipped with:
        ///
        /// - Task-local data
        /// - A means of waking the task
        /// - A means of spawning new tasks, i.e. an [executor]()
        pub fn new(map: &'a mut LocalMap, waker: &'a Waker, executor: &'a mut Executor) -> Context<'a> {
            Context { waker, map, executor }
        }

        /// Get the default executor associated with this task.
        ///
        /// This method is useful primarily if you want to explicitly handle
        /// spawn failures.
        pub fn executor(&mut self) -> &mut Executor {
            self.executor
        }

        /// Spawn a future onto the default executor.
        ///
        /// # Panics
        ///
        /// This method will panic if the default executor is unable to spawn.
        /// To handle executor errors, use [executor()](self::Context::executor)
        /// instead.
        pub fn spawn<F>(&mut self, f: F)
            where F: Future<Item = (), Error = Never> + 'static + Send
        {
            self.executor.spawn(Box::new(f)).unwrap()
        }
    }
}

if_not_std! {
    impl<'a> Context<'a> {
        /// Create a new task context.
        ///
        /// In no_std mode, task contexts are always equipped with:
        ///
        /// - Task-local data
        /// - A means of waking the task
        pub fn new(map: &'a mut LocalMap, waker: &'a Waker) -> Context<'a> {
            Context { waker, map, executor: () }
        }
    }
}

impl<'a> fmt::Debug for Context<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Context")
         .finish()
    }
}

/// A map storing task-local data.
pub struct LocalMap {
    #[allow(dead_code)]
    inner: data::LocalMap,
}

impl LocalMap {
    /// Create an empty set of task-local data.
    pub fn new() -> LocalMap {
        LocalMap { inner: data::local_map() }
    }
}

impl fmt::Debug for LocalMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("LocalMap")
         .finish()
    }
}

/// An unsafe trait for implementing custom memory management for a
/// [`Waker`](::task::Waker).
///
/// A [`Waker`](::task::Waker) is a cloneable trait object for `Wake`, and is
/// most often essentially just `Arc<Wake>`. However, in some contexts
/// (particularly `no_std`), it's desirable to avoid `Arc` in favor of some
/// custom memory management strategy. This trait is designed to allow for such
/// customization.
///
/// A default implementation of the `UnsafeWake` trait is provided for the
/// `Arc` type in the standard library. If the `std` feature of this crate
/// is not available however, you'll be required to implement your own
/// instance of this trait to pass it into `Waker::new`.
///
/// # Unsafety
///
/// This trait manually encodes the memory management of the underlying trait
/// object. Implementors of this trait must guarantee:
///
/// * Calls to `clone_raw` produce uniquely owned `Waker` handles. These handles
/// should be independently usable and droppable.
///
/// * Calls to `drop_raw` work with `self` as a raw pointer, deallocating
///   resources associated with it. This is a pretty unsafe operation as it's
///   invalidating the `self` pointer, so extreme care needs to be taken.
///
/// In general it's recommended to review the trait documentation as well as the
/// implementation for `Arc` in this crate before attempting a custom
/// implementation.
pub unsafe trait UnsafeWake {
    /// Creates a new `Waker` from this instance of `UnsafeWake`.
    ///
    /// This function will create a new uniquely owned handle that under the
    /// hood references the same notification instance. In other words calls
    /// to `wake` on the returned handle should be equivalent to calls to
    /// `wake` on this handle.
    ///
    /// # Unsafety
    ///
    /// This is also unsafe to call because it's asserting the `UnsafeWake`
    /// value is in a consistent state, i.e. hasn't been dropped.
    unsafe fn clone_raw(&self) -> Waker;

    /// Drops this instance of `UnsafeWake`, deallocating resources
    /// associated with it.
    ///
    /// This method is intended to have a signature such as:
    ///
    /// ```ignore
    /// fn drop_raw(self: *mut Self);
    /// ```
    ///
    /// Unfortunately in Rust today that signature is not object safe.
    /// Nevertheless it's recommended to implement this function *as if* that
    /// were its signature. As such it is not safe to call on an invalid
    /// pointer, nor is the validity of the pointer guaranteed after this
    /// function returns.
    ///
    /// # Unsafety
    ///
    /// This is also unsafe to call because it's asserting the `UnsafeWake`
    /// value is in a consistent state, i.e. hasn't been dropped
    unsafe fn drop_raw(&self);

    /// Indicates that the associated task is ready to make progress and should
    /// be `poll`ed.
    ///
    /// Executors generally maintain a queue of "ready" tasks; `wake` should place
    /// the associated task onto this queue.
    ///
    /// # Panics
    ///
    /// Implementations should avoid panicking, but clients should also be prepared
    /// for panics.
    ///
    /// # Unsafety
    ///
    /// This is also unsafe to call because it's asserting the `UnsafeWake`
    /// value is in a consistent state, i.e. hasn't been dropped
    unsafe fn wake(&self);
}

/// A `Waker` is a handle for waking up a task by notifying its executor that it
/// is ready to be run.
///
/// This handle contains a trait object pointing to an instance of the `Wake`
/// trait, allowing notifications to get routed through it. Usually `Waker`
/// instances are provided by an executor.
///
/// If you're implementing an executor, the recommended way to create a `Waker`
/// is via `Waker::from` applied to an `Arc<T>` value where `T: Wake`. The
/// unsafe `new` constructor should be used only in niche, `no_std` settings.
pub struct Waker {
    inner: *mut UnsafeWake,
}

unsafe impl Send for Waker {}
unsafe impl Sync for Waker {}

impl Waker {
    /// Constructs a new `Waker` directly.
    ///
    /// Note that most code will not need to call this. Implementers of the
    /// `UnsafeWake` trait will typically provide a wrapper that calls this
    /// but you otherwise shouldn't call it directly.
    ///
    /// If you're working with the standard library then it's recommended to
    /// use the `Waker::from` function instead which works with the safe
    /// `Arc` type and the safe `Wake` trait.
    #[inline]
    pub unsafe fn new(inner: *mut UnsafeWake) -> Waker {
        Waker { inner: inner }
    }

    /// Wake up the task associated with this `Waker`.
    pub fn wake(&self) {
        unsafe { (*self.inner).wake() }
    }

    /// Returns whether or not this `Waker` and `other` awaken the same task.
    ///
    /// This function works on a best-effort basis, and may return false even
    /// when the `Waker`s would awaken the same task. However, if this function
    /// returns true, it is guaranteed that the `Waker`s will awaken the same
    /// task.
    ///
    /// This function is primarily used for optimization purposes.
    pub fn will_wake(&self, other: &Waker) -> bool {
        self.inner == other.inner
    }
}

impl Clone for Waker {
    #[inline]
    fn clone(&self) -> Self {
        unsafe {
            (*self.inner).clone_raw()
        }
    }
}

impl fmt::Debug for Waker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Waker")
         .finish()
    }
}

impl Drop for Waker {
    fn drop(&mut self) {
        unsafe {
            (*self.inner).drop_raw()
        }
    }
}

if_std! {
    use std::mem;
    use std::ptr;
    use std::sync::Arc;

    mod data;

    pub use self::data::LocalKey;

    /// A way of waking up a specific task.
    ///
    /// Any task executor must provide a way of signaling that a task it owns
    /// is ready to be `poll`ed again. Executors do so by implementing this trait.
    ///
    /// Note that, rather than working directly with `Wake` trait objects, this
    /// library instead uses a custom [`Waker`](::task::Waker) to allow for
    /// customization of memory management.
    pub trait Wake: Send + Sync {
        /// Indicates that the associated task is ready to make progress and should
        /// be `poll`ed.
        ///
        /// Executors generally maintain a queue of "ready" tasks; `wake` should place
        /// the associated task onto this queue.
        ///
        /// # Panics
        ///
        /// Implementations should avoid panicking, but clients should also be prepared
        /// for panics.
        fn wake(arc_self: &Arc<Self>);
    }

    // Safe implementation of `UnsafeWake` for `Arc` in the standard library.
    //
    // Note that this is a very unsafe implementation! The crucial pieces is that
    // these two values are considered equivalent:
    //
    // * Arc<T>
    // * *const ArcWrapped<T>
    //
    // We don't actually know the layout of `ArcWrapped<T>` as it's an
    // implementation detail in the standard library. We can work, though, by
    // casting it through and back an `Arc<T>`.
    //
    // This also means that you won't actually find `UnsafeWake for Arc<T>`
    // because it's the wrong level of indirection. These methods are sort of
    // receiving Arc<T>, but not an owned version. It's... complicated. We may be
    // one of the first users of unsafe trait objects!

    struct ArcWrapped<T>(PhantomData<T>);

    unsafe impl<T: Wake + 'static> UnsafeWake for ArcWrapped<T> {
        unsafe fn clone_raw(&self) -> Waker {
            let me: *const ArcWrapped<T> = self;
            let arc = (*(&me as *const *const ArcWrapped<T> as *const Arc<T>)).clone();
            Waker::from(arc)
        }

        unsafe fn drop_raw(&self) {
            let mut me: *const ArcWrapped<T> = self;
            let me = &mut me as *mut *const ArcWrapped<T> as *mut Arc<T>;
            ptr::drop_in_place(me);
        }

        unsafe fn wake(&self) {
            let me: *const ArcWrapped<T> = self;
            T::wake(&*(&me as *const *const ArcWrapped<T> as *const Arc<T>))
        }
    }

    impl<T> From<Arc<T>> for Waker
        where T: Wake + 'static,
    {
        fn from(rc: Arc<T>) -> Waker {
            unsafe {
                let ptr = mem::transmute::<Arc<T>, *mut ArcWrapped<T>>(rc);
                Waker::new(ptr)
            }
        }
    }
}

#[cfg(not(feature = "std"))]
mod data {
    pub struct LocalMap;

    pub fn local_map() -> LocalMap {
        LocalMap
    }
}
