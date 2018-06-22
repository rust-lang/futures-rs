use std::marker::Unpin;
use std::mem::PinMut;

use super::iter_pin_mut::IterPinMut;

#[derive(Debug)]
/// Mutable iterator over all futures in the unordered set.
pub struct IterMut<'a, F: 'a + Unpin> (pub(super) IterPinMut<'a, F>);

impl<'a, F: Unpin> Iterator for IterMut<'a, F> {
    type Item = &'a mut F;

    fn next(&mut self) -> Option<&'a mut F> {
        self.0.next().map(|f| unsafe { PinMut::get_mut(f) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a, F: Unpin> ExactSizeIterator for IterMut<'a, F> {}
