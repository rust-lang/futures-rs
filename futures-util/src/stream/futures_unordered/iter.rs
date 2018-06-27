use std::marker::{PhantomData, Unpin};
use std::mem::PinMut;

use super::FuturesUnordered;
use super::node::Node;

#[derive(Debug)]
/// Mutable iterator over all futures in the unordered set.
pub struct IterPinMut<'a, F: 'a> {
    pub(super) node: *const Node<F>,
    pub(super) len: usize,
    pub(super) _marker: PhantomData<&'a mut FuturesUnordered<F>>
}

#[derive(Debug)]
/// Mutable iterator over all futures in the unordered set.
pub struct IterMut<'a, F: 'a + Unpin> (pub(super) IterPinMut<'a, F>);

impl<'a, F> Iterator for IterPinMut<'a, F> {
    type Item = PinMut<'a, F>;

    fn next(&mut self) -> Option<PinMut<'a, F>> {
        if self.node.is_null() {
            return None;
        }
        unsafe {
            let future = (*(*self.node).future.get()).as_mut().unwrap();
            let next = *(*self.node).next_all.get();
            self.node = next;
            self.len -= 1;
            Some(PinMut::new_unchecked(future))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, F> ExactSizeIterator for IterPinMut<'a, F> {}

impl<'a, F: Unpin> Iterator for IterMut<'a, F> {
    type Item = &'a mut F;

    fn next(&mut self) -> Option<&'a mut F> {
        self.0.next().map(|f| PinMut::get_mut(f))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a, F: Unpin> ExactSizeIterator for IterMut<'a, F> {}
