use std::sync::{Arc, Weak};
use std::cell::UnsafeCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::mem;

use futures::{Future, Poll, Executor};
use futures::task::{Task, Unpark};

use super::{Message, LoopHandle, LoopPin};

/// A running task on the event loop executor
pub struct LoopTask {
    handle: LoopHandle<'static>,
    inner: UnsafeCell<Option<Inner>>,

    // When we want to run the task, we'll need an `Arc<Unpark>` to set on
    // entry. We can't use the one used to wake the task in the first place,
    // because `Unpark` itself works by reference. And we can't use a full
    // `Arc<Unpark>` here, or we'd have a cycle. A weak reference works well,
    // however: there will always be at least one strong reference as long as
    // the task is either running or scheduled to be polled somewhere -- and if
    // neither of those things is true, then the task *should* be dropped.
    //
    // Note that after it is set, `cycle` is never changed. The reason it is
    // wrapped in an UnsafeCell and Option is that we have to construct the
    // cycle after initially constructing a LoopTask and placing that within an
    // Arc.
    cycle: UnsafeCell<Option<Weak<LoopTask>>>,
}

// A LoopTask is effectively a handle for waking up a task *on the event
// loop*. Despite the fact that it stores data that is not Send/Sync, we
// guarantee that the data is accessed only according to Rust's usual rules.
//
// In particular, after construction, `cycle` is immutable and is effectively
// Send + Sync.
//
// On the other hand, `inner` is may not be Send/Sync, but the LoopTask will
// guarantee that it is only accessed on the event loop thread (which must
// likewise be where the future's data is pinned -- see LoopPin below).
unsafe impl Send for LoopTask {}
unsafe impl Sync for LoopTask {}

// The guts of LoopTask: the task, and the future it's executing
struct Inner {
    task: Task,

    // Safety note: despite the fact that this data is 'static, it may actually
    // embed non-'static data. This is safe because the future will only be
    // polled by the event loop, which is guaranteed to outlive any data
    // referenced by this task. To see why, see the Executor impls below.
    fut: Box<Future<Item = (), Error = ()>>,
}

/// Actually poll the task.
///
/// For safety, this must be called only by the event loop, in a
/// single-threaded, non-reentrant way.
pub unsafe fn run(loop_task: Arc<LoopTask>) {
    let inner = loop_task.inner.get();
    let unpark: Arc<Unpark> = loop_task;

    if let Some(Inner { mut task, mut fut }) = (*inner).take() {
        // TODO: add hooks to avoid dropping the panic on the floor
        let res = catch_unwind(AssertUnwindSafe(|| {
            task.enter(&unpark, || fut.poll())
        }));

        // if we have more polling to do, reinstall our guts for the next round
        if let Ok(Poll::NotReady) = res {
            *inner = Some(Inner { task: task, fut: fut });
        }
    }
}

impl Unpark for LoopTask {
    fn unpark(&self) {
        let cycle = unsafe { (*self.cycle.get()).as_ref().unwrap() };
        let task = Weak::upgrade(cycle).unwrap();
        self.handle.send(Message::RunTask(task));
    }
}

// The use of 'a here is justified by the fact that the event loop -- the only
// thing to actually poll the future -- is guaranteed to outlive 'a.
impl<'a, F: Future + Send + 'a> Executor<F> for LoopHandle<'a> {
    fn spawn(&self, f: F) {
        unsafe { spawn(f, self.clone()) }
    }
}

// The use of 'a here is justified by the fact that the event loop -- the only
// thing to actually poll the future -- is guaranteed to outlive 'a. The lack of
// Send is justified by the fact that we have in our hand a `LoopPin`, which
// guarantees that the future is owned by the same thread that will execute the
// event loop.
impl<'a, F: Future + 'a> Executor<F> for LoopPin<'a> {
    fn spawn(&self, f: F) {
        unsafe { spawn(f, self.handle()) }
    }
}

/// Spawn a new LoopTask. Unsafe because it does not check the correctness of
/// any Send bounds; these are checked by the Executor impls above.
unsafe fn spawn<'a, F: Future + 'a>(f: F, handle: LoopHandle<'a>) {
    let boxed = Box::new(f.map(|_| ()).map_err(|_| ()));
    let boxed_fut: Box<Future<Item = (), Error = ()> + 'a> = boxed;
    // Erase the lifetime! See the comment on `Inner` for details
    let boxed_fut: Box<Future<Item = (), Error = ()>> = mem::transmute(boxed_fut);

    let task = Arc::new(LoopTask {
        handle: handle.clone().into_static(),
        cycle: UnsafeCell::new(None),
        inner: UnsafeCell::new(Some(Inner {
            task: Task::new(),
            fut: boxed_fut,
        }))
    });
    *task.cycle.get() = Some(Arc::downgrade(&task));
    handle.send(Message::RunTask(task))
}
