#[macro_export]
macro_rules! unsafe_pinned {
    ($f:tt -> $t:ty) => (
        fn $f<'a>(
            self: &'a mut $crate::core_reexport::mem::PinMut<Self>
        ) -> $crate::core_reexport::mem::PinMut<'a, $t> {
            unsafe {
                $crate::core_reexport::mem::PinMut::map_unchecked(
                    self.reborrow(), |x| &mut x.$f
                )
            }
        }
    )
}

#[macro_export]
macro_rules! unsafe_unpinned {
    ($f:tt -> $t:ty) => (
        fn $f<'a>(
            self: &'a mut $crate::core_reexport::mem::PinMut<Self>
        ) -> &'a mut $t {
            unsafe {
                &mut $crate::core_reexport::mem::PinMut::get_mut_unchecked(
                    self.reborrow()
                ).$f
            }
        }
    )
}

#[macro_export]
macro_rules! pin_mut {
    ($($x:ident),*) => { $(
        // Move the value to ensure that it is owned
        let mut $x = $x;
        // Shadow the original binding so that it can't be directly accessed
        // ever again.
        #[allow(unused_mut)]
        let mut $x = unsafe {
            $crate::core_reexport::mem::PinMut::new_unchecked(&mut $x)
        };
    )* }
}
