extern crate futures_core;
extern crate futures_util;

use futures_core::task::NotifyHandle;

#[derive(Clone)]
struct IntoNotifyHandle<'a>(&'a (Fn() -> NotifyHandle + 'a));

impl<'a> From<IntoNotifyHandle<'a>> for NotifyHandle {
    fn from(handle: IntoNotifyHandle<'a>) -> NotifyHandle {
        (handle.0)()
    }
}

mod thread;
mod task_runner;

pub mod current_thread;

mod enter;
pub use enter::{enter, Enter, EnterError};
