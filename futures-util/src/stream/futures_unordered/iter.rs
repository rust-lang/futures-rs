use super::FuturesUnordered;
use super::task::Task;
use std::marker::{PhantomData, Unpin};
use std::mem::PinMut;

#[derive(Debug)]
/// Mutable iterator over all futures in the unordered set.
pub struct IterPinMut<'a, Fut: 'a> {
    pub(super) task: *const Task<Fut>,
    pub(super) len: usize,
    pub(super) _marker: PhantomData<&'a mut FuturesUnordered<Fut>>
}

#[derive(Debug)]
/// Mutable iterator over all futures in the unordered set.
pub struct IterMut<'a, Fut: 'a + Unpin> (pub(super) IterPinMut<'a, Fut>);

impl<'a, Fut> Iterator for IterPinMut<'a, Fut> {
    type Item = PinMut<'a, Fut>;

    fn next(&mut self) -> Option<PinMut<'a, Fut>> {
        if self.task.is_null() {
            return None;
        }
        unsafe {
            let future = (*(*self.task).future.get()).as_mut().unwrap();
            let next = *(*self.task).next_all.get();
            self.task = next;
            self.len -= 1;
            Some(PinMut::new_unchecked(future))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, Fut> ExactSizeIterator for IterPinMut<'a, Fut> {}

impl<'a, Fut: Unpin> Iterator for IterMut<'a, Fut> {
    type Item = &'a mut Fut;

    fn next(&mut self) -> Option<&'a mut Fut> {
        self.0.next().map(PinMut::get_mut)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a, Fut: Unpin> ExactSizeIterator for IterMut<'a, Fut> {}
