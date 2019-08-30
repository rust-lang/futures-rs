use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll, Waker, RawWaker, RawWakerVTable};

/// Handle to a future spawned on a [InertExecutor](self::InertExecutor).
pub(crate) struct InertTaskHandle<T>(usize, Arc<Mutex<Option<T>>>);

/// A future executor which is totally inert, making it useful for testing future
/// implementations. It spawns no threads, and all progress is driven by explicit control
/// methods such as [poll_one](self::InertExecutor::poll_one()).
pub(crate) struct InertExecutor(Arc<Mutex<InertExecutorInner>>);

struct InertExecutorInner {
    next_id: usize,
    tasks: HashMap<usize, Box<dyn InertTask>>,
    wakes: Vec<usize>,
    completed: Vec<usize>
}

impl InertExecutor {
    /// Create a new [InertExecutor](self::InertExecutor). Absolutely no threads were
    /// spawned in the making of this executor.
    pub(crate) fn new() -> Self {
        Self(Arc::new(Mutex::new(InertExecutorInner {
            next_id: 1,
            tasks: HashMap::new(),
            wakes: Vec::new(),
            completed: Vec::new()
        })))
    }

    /// Spawn a [Future](std::future::Future) onto the inert executor, returning a handle
    /// to the spawned task.
    pub(crate) fn spawn<F: 'static + Future>(&self, future: F) -> InertTaskHandle<F::Output> {
        let mut inner = self.0.lock().unwrap();
        let id = inner.next_id;
        inner.next_id += 1;

        let complete = Arc::new(Mutex::new(None));
        let task = InertTaskImpl {
            future,
            complete: complete.clone()
        };

        inner.tasks.insert(id, Box::new(task));
        InertTaskHandle(id, complete)
    }

    /// Removes completed tasks from the executor. Completed tasks are tasks which have
    /// previously returned Poll::Ready when polled.
    pub(crate) fn clear_completed(&self) {
        let mut inner = self.0.lock().unwrap();
        let mut completed = std::mem::replace(&mut inner.completed, Vec::with_capacity(0));
        completed.retain(|task| {
            inner.tasks.remove(task);
            false
        });
        std::mem::replace(&mut inner.completed, completed);
    }

    /// Polls all tasks which woke their waker, and clears their wake status.
    pub(crate) fn poll_woken(&self) {
        let mut inner = self.0.lock().unwrap();
        let wakes = inner.wakes.clone();
        inner.wakes.clear();
        for task in wakes {
            if let Some(inert_task) = inner.tasks.get_mut(&task) {
                let waker = self.waker(task);
                let mut context = Context::from_waker(&waker);
                if inert_task.poll(&mut context) {
                    inner.completed.push(task);
                }
            }
        }
    }

    /// Number of times the given task has been woken. Note that poll_woken resets this
    /// count, and poll_one decrements it by 1.
    pub(crate) fn wake_count<T>(&self, task_handle: &InertTaskHandle<T>) -> usize {
        self.0.lock().unwrap()
            .wakes.iter()
            .filter(|&&task| task == task_handle.0)
            .count()
    }

    /// Poll a single task, and decrement its wake count (if any) by 1. Returns the result
    /// of polling the task.
    pub(crate) fn poll_one<T>(&self, task_handle: &InertTaskHandle<T>) -> Poll<T> {
        let mut inner = self.0.lock().unwrap();
        if let Some(pos) = inner.wakes.iter().position(|&task| task == task_handle.0) {
            inner.wakes.remove(pos);
        }
        let inert_task = inner.tasks.get_mut(&task_handle.0).expect("Task not found");
        let waker = self.waker(task_handle.0);
        let mut context = Context::from_waker(&waker);
        if inert_task.poll(&mut context) {
            inner.completed.push(task_handle.0);
            Poll::Ready(task_handle.1.lock().unwrap().take().unwrap())
        } else {
            Poll::Pending
        }
    }

    fn waker(&self, task: usize) -> Waker {
        let waker_data = Box::leak(Box::new(InertWaker {
            executor: Arc::downgrade(&self.0),
            task
        }));
        let raw_waker = RawWaker::new(waker_data as *const _ as *const (), &INERT_WAKER_VTABLE);
        unsafe { Waker::from_raw(raw_waker) }
    }
}

#[derive(Clone)]
struct InertWaker {
    executor: Weak<Mutex<InertExecutorInner>>,
    task: usize
}

static INERT_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    inert_waker_clone,
    inert_waker_wake,
    inert_waker_wake_by_ref,
    inert_waker_drop
);

unsafe fn inert_waker_clone(data: *const ()) -> RawWaker {
    let waker_data: &mut InertWaker = &mut *(data as *mut () as *mut _);
    let cloned: &mut InertWaker = Box::leak(Box::new(waker_data.clone()));
    RawWaker::new(cloned as *const _ as *const (), &INERT_WAKER_VTABLE)
}

unsafe fn inert_waker_wake(data: *const ()) {
    let waker_data: Box<InertWaker> = Box::from_raw(&mut *(data as *mut () as *mut _));
    if let Some(mutex) = waker_data.executor.upgrade() {
        let mut inner = mutex.lock().unwrap();
        inner.wakes.push(waker_data.task);
    }
}

unsafe fn inert_waker_wake_by_ref(data: *const ()) {
    let waker_data: &mut InertWaker = &mut *(data as *mut () as *mut _);
    if let Some(mutex) = waker_data.executor.upgrade() {
        let mut inner = mutex.lock().unwrap();
        inner.wakes.push(waker_data.task);
    }
}

unsafe fn inert_waker_drop(data: *const () ) {
    Box::from_raw(&mut *(data as *mut () as *mut _));
}

trait InertTask {
    fn poll(&mut self, cx: &mut Context<'_>) -> bool;
}

struct InertTaskImpl<F: Future> {
    future: F,
    complete: Arc<Mutex<Option<F::Output>>>
}

impl<F: Future> InertTask for InertTaskImpl<F> {
    fn poll(&mut self, cx: &mut Context<'_>) -> bool {
        // I have no idea if this is safe or not. There is supposed to be an Unpin impl
        // for impl<'a, T> Unpin for &'a mut T where T: 'a + ?Sized, but it wasn't working
        // to call Pin::new.
        let fut_pin = unsafe { Pin::new_unchecked(&mut self.future) };
        match fut_pin.poll(cx) {
            Poll::Ready(completion_value) => {
                self.complete.lock().unwrap().replace(completion_value);
                true
            },
            Poll::Pending => {
                false
            }
        }
    }
}

#[cfg(test)]
mod inert_tests {
    use super::InertExecutor;
    use std::task::Poll;

    #[test]
    fn test_inert_executor() {
        let executor = InertExecutor::new();
        let future = crate::future::ready(123);
        let task = executor.spawn(future);
        assert_eq!(0, executor.wake_count(&task));
        assert_eq!(Poll::Ready(123), executor.poll_one(&task));
        assert_eq!(0, executor.wake_count(&task));
        executor.poll_woken();
        assert_eq!(0, executor.wake_count(&task));
        executor.clear_completed();
    }
}