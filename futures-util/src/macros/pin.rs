/// A pinned projection of a struct field.
///
/// To make using this macro safe, two things need to be ensured:
/// - If the struct implements [`Drop`], the [`drop`] method is not allowed to
///   move the value of the field.
/// - If the struct wants to implement [`Unpin`], it has to do so conditionally:
///   The struct can only implement [`Unpin`] if the field's type is [`Unpin`].
///
/// ```
/// # #![feature(pin, arbitrary_self_types)]
/// # #[macro_use] extern crate futures;
/// # use core::mem::PinMut;
/// # use core::marker::Unpin;
/// struct Foo<T> {
///     field: T,
/// }
///
/// impl<T> Foo<T> {
///     unsafe_pinned!(field: T);
///
///     fn baz(mut self: PinMut<Self>) {
///         let _: PinMut<T> = self.field(); // Pinned reference to the field
///     }
/// }
///
/// impl<T: Unpin> Unpin for Foo<T> {}; // Conditional Unpin impl
/// ```
///
/// [`Unpin`]: std::marker::Unpin
/// [`drop`]: Drop::drop
#[macro_export]
macro_rules! unsafe_pinned {
    ($f:tt: $t:ty) => (
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

/// An unpinned projection of a struct field.
///
/// This macro is unsafe because it creates a method that returns a normal
/// non-pin reference to the struct field. It is up to the programmer to ensure
/// that the contained value can be considered not pinned in the current
/// context.
///
/// ```
/// # #![feature(pin, arbitrary_self_types)]
/// # #[macro_use] extern crate futures;
/// # use core::mem::PinMut;
/// # struct Bar;
/// struct Foo {
///     field: Bar,
/// }
///
/// impl Foo {
///     unsafe_unpinned!(field: Bar);
///
///     fn baz(mut self: PinMut<Self>) {
///         let _: &mut Bar = self.field(); // Normal reference to the field
///     }
/// }
/// ```
#[macro_export]
macro_rules! unsafe_unpinned {
    ($f:tt: $t:ty) => (
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

/// Pins a value on the stack.
///
/// ```
/// # #![feature(pin, arbitrary_self_types)]
/// # #[macro_use] extern crate futures;
/// # use core::mem::PinMut;
/// # struct Foo {}
/// let foo = Foo { /* ... */ };
/// pin_mut!(foo);
/// let _: PinMut<Foo> = foo;
/// ```
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
