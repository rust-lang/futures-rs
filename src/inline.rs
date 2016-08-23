use {Future, Poll, Executor};
use task::{Task, Unpark};

use std::prelude::v1::*;
use std::cell::UnsafeCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Weak};

/// An `Executor` that will continually execute a task "on the spot", on
/// whatever thread happened to notify it.
// TODO: add options for control over panic behavior
pub struct Inline;

impl<F: Future + Send + 'static> Executor<F> for Inline {
    fn spawn(&self, f: F) {
        let task = InlineTask {
            status: AtomicUsize::new(POLLING),
            inner: UnsafeCell::new(None),
        };

        // safe because we are in the POLLING state and are responsible for
        // entering it.
        unsafe {
            let task = Arc::new(task);
            let unpark = Arc::downgrade(&task);
            *task.inner.get() = Some(Inner {
                unpark: unpark,
                task: Task::new(),
                fut: f,
            });
            task.run();
        };
    }
}

/// A running task on the inline executor, with future type `F`
struct InlineTask<F> {
    // The state of task execution (state machine described below)
    status: AtomicUsize,

    // The actual task data, accessible only in the POLLING state
    inner: UnsafeCell<Option<Inner<F>>>,
}

// `InlineTask<F>` functions in many ways like a `Mutex<F>`, except that on
// acquisition failure, the current lockholder performs the desired work --
// re-polling.
//
// As such, these impls mirror those for `Mutex<F>`. In particular, a reference
// to `InlineTask` can be used to gain `&mut` access to the inner data, which
// must therefore be `Send`.
unsafe impl<F: Send> Send for InlineTask<F> {}
unsafe impl<F: Send> Sync for InlineTask<F> {}

// There are for possible task states, listed below with their possible
// transitions:

// The task is blocked, waiting on an event
const WAITING: usize = 0;       // --> POLLING

// The task is actively being polled by a thread; arrival of additional events
// of interest should move it to the REPOLL state
const POLLING: usize = 1;       // --> WAITING, REPOLL, or COMPLETE

// The task is actively being polled, but will need to be re-polled upon
// completion to ensure that all events were observed.
const REPOLL: usize = 2;        // --> POLLING

// The task has finished executing (either successfully or with an error/panic)
const COMPLETE: usize = 3;      // No transitions out

// The guts of a InlineTask: the generic task data and the future
struct Inner<F> {
    // When we want to run the task, we'll need an `Arc<Unpark>` to set on
    // entry. We can't use the one used to wake the task in the first place,
    // because `Unpark` itself works by reference. And we can't use a full
    // `Arc<Unpark>` here, or we'd have a cycle. A weak reference works well,
    // however: there will always be at least one strong reference as long as
    // the task is either running or scheduled to be polled somewhere -- and if
    // neither of those things is true, then the task *should* be dropped.
    unpark: Weak<Unpark>,
    task: Task,
    fut: F,
}

impl<F: Future + Send + 'static> InlineTask<F> {
    // Actually execute the task. Safety requires that this function is only
    // when the `inner` data is uniquely owned. Thus it must only be called in
    // the POLLING state, and can only be called by the thread that transitioned
    // to that state.
    unsafe fn run(&self) {
        loop {
            let Inner { mut task, mut fut, unpark } = (*self.inner.get()).take().unwrap();

            // TODO: add hooks to avoid dropping the panic on the floor
            let res = catch_unwind(AssertUnwindSafe(|| {
                task.enter(&Weak::upgrade(&unpark).unwrap(), || fut.poll())
            }));

            if let Ok(Poll::NotReady) = res {
                *self.inner.get() = Some(Inner { task: task, fut: fut, unpark: unpark });

                // check whether we need to re-poll
                if self.status.compare_exchange(POLLING, WAITING, SeqCst, SeqCst).is_ok() {
                    return
                } else {
                    // guaranteed to be in REPOLL state; just clobber the state and run again.
                    self.status.store(POLLING, SeqCst);
                }
            } else {
                // drop `inner` on the floor, deallocating the data ASAP
                self.status.store(COMPLETE, SeqCst);
                return;
            }
        }
    }
}

impl<F: Future + Send + 'static> Unpark for InlineTask<F> {
    fn unpark(&self) {
        loop {
            match self.status.load(SeqCst) {
                // The task is idle, so try to run it immediately.
                WAITING => {
                    if self.status.compare_exchange(WAITING, POLLING, SeqCst, SeqCst).is_ok() {
                        // Uinque access to the inner data is ensured through
                        // the mutual exclusion protocol of the state machine:
                        // we wre the sole thread that has reached the POLLING
                        // state.
                        unsafe { self.run(); }
                        return
                    }
                }

                // The task is being polled, so we need to record that it should be *repolled*
                // when complete.
                POLLING => {
                    if self.status.compare_exchange(POLLING, REPOLL, SeqCst, SeqCst).is_ok() {
                        return
                    }
                }

                // The task is already scheduled for polling, so we've got nothing to do.
                REPOLL => return,

                // The task is already finished.
                COMPLETE => return,

                _ => unreachable!()
            }
        }
    }
}
