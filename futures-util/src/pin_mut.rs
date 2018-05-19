use core::mem::PinMut;

/// Useful additions to the `PinMut` type
pub trait PinMutExt<T> {
    /// Overwrite the referenced data, dropping the existing content
    fn assign(&mut self, data: T);
}

impl<'a, T> PinMutExt<T> for PinMut<'a, T> {
    fn assign(&mut self, data: T) {
        unsafe { *PinMut::get_mut(self.reborrow()) = data }
    }
}

/// Useful additions to the `Option` type, for working with `PinMut`
pub trait OptionExt<'a, T> {
    /// Push `PinMut` through an `Option`
    fn as_pin_mut(self) -> Option<PinMut<'a, T>>;
}

impl<'a, T> OptionExt<'a, T> for PinMut<'a, Option<T>> {
    fn as_pin_mut(self) -> Option<PinMut<'a, T>> {
        unsafe {
            PinMut::get_mut(self).as_mut().map(|x| {
                PinMut::new_unchecked(x)
            })
        }
    }
}
