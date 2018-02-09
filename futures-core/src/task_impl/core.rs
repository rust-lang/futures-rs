#![cfg_attr(feature = "std", allow(dead_code))]

use core::marker;

use super::NotifyHandle;

pub struct LocalMap;
pub fn local_map() -> LocalMap { LocalMap }

#[derive(Copy, Clone)]
pub struct BorrowedEvents<'a>(marker::PhantomData<&'a ()>);

#[derive(Copy, Clone)]
pub struct BorrowedUnpark<'a> {
    f: &'a Fn() -> NotifyHandle,
    id: usize,
}

pub struct TaskUnpark {
    handle: NotifyHandle,
    id: usize,
}

impl<'a> BorrowedUnpark<'a> {
    #[inline]
    pub fn new(f: &'a Fn() -> NotifyHandle, id: usize) -> BorrowedUnpark<'a> {
        BorrowedUnpark { f: f, id: id }
    }

    #[inline]
    pub fn to_owned(&self) -> TaskUnpark {
        let handle = (self.f)();
        let id = handle.clone_id(self.id);
        TaskUnpark { handle: handle, id: id }
    }
}

impl TaskUnpark {
    pub fn notify(&self) {
        self.handle.notify(self.id);
    }

    pub fn will_notify(&self, other: &BorrowedUnpark) -> bool {
        self.id == other.id && self.handle.inner == (other.f)().inner
    }
}

impl Clone for TaskUnpark {
    fn clone(&self) -> TaskUnpark {
        let handle = self.handle.clone();
        let id = handle.clone_id(self.id);
        TaskUnpark { handle: handle, id: id }
    }
}

impl Drop for TaskUnpark {
    fn drop(&mut self) {
        self.handle.drop_id(self.id);
    }
}
