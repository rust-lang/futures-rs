use std::sync::Arc;
use std::thread::{self, Thread};

use futures_core::task::Wake;

pub(crate) struct ThreadNotify {
    thread: Thread,
}

thread_local! {
    static CURRENT_THREAD_NOTIFY: Arc<ThreadNotify> = Arc::new(ThreadNotify {
        thread: thread::current(),
    });
}

impl ThreadNotify {
    pub(crate) fn with_current<R, F>(f: F) -> R
        where F: FnOnce(&Arc<ThreadNotify>) -> R,
    {
        CURRENT_THREAD_NOTIFY.with(f)
    }

    pub(crate) fn park(&self) {
        thread::park();
    }
}

impl Wake for ThreadNotify {
    fn wake(arc_self: &Arc<Self>) {
        arc_self.thread.unpark();
    }
}
