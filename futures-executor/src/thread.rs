use std::sync::Arc;
use std::thread::{self, Thread};

use futures_core::task::Wake;

pub struct ThreadNotify {
    thread: Thread,
}

thread_local! {
    static CURRENT_THREAD_NOTIFY: Arc<ThreadNotify> = Arc::new(ThreadNotify {
        thread: thread::current(),
    });
}

impl ThreadNotify {
    pub fn with_current<F, R>(f: F) -> R
        where F: FnOnce(&Arc<ThreadNotify>) -> R,
    {
        CURRENT_THREAD_NOTIFY.with(f)
    }

    pub fn park(&self) {
        thread::park();
    }
}

impl Wake for ThreadNotify {
    fn wake(&self) {
        self.thread.unpark();
    }
}
