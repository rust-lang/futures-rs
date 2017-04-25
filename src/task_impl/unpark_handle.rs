use core::ptr;
use core::mem::{self, forget, size_of};
use core::marker::PhantomData;
use core::cell::Cell;
use super::Unpark;

// Stolen from https://gist.github.com/alexcrichton/29b618d75cde5b57d797
macro_rules! cascade {
    ($(
        if #[cfg($($meta:meta),*)] { $($it:item)* }
    ) else * else {
        $($it2:item)*
    }) => {
        __items! {
            () ;
            $( ( ($($meta),*) ($($it)*) ), )*
            ( () ($($it2)*) ),
        }
    }
}

macro_rules! __items {
    (($($not:meta,)*) ; ) => {};
    (($($not:meta,)*) ; ( ($($m:meta),*) ($($it:item)*) ), $($rest:tt)*) => {
        __apply! { cfg(all($($m,)* not(any($($not),*)))), $($it)* }
        __items! { ($($not,)* $($m,)*) ; $($rest)* }
    }
}

macro_rules! __apply {
    ($m:meta, $($it:item)*) => {
        $(#[$m] $it)*
    }
}

cascade! {
    if #[cfg(feature="MaxUnparkBytes256")] {
        const _MAX_UNPARK_BYTES: usize = 256;
    } else if #[cfg(feature="MaxUnparkBytes128")] {
        const _MAX_UNPARK_BYTES: usize = 128;
    } else {
        const _MAX_UNPARK_BYTES: usize = 64;
    }
}

/// Maximum size of `T` in `UnparkHandle<T>`, in bytes.
/// Configurable by the MaxUnparkBytes features.
pub const MAX_UNPARK_BYTES : usize = _MAX_UNPARK_BYTES;

/// A VTable that knows how to clone because the data has a maximum size.
#[derive(Copy)]
struct UnparkVtable {
    unpark: unsafe fn(*const ()),
    clone_to_buffer: unsafe fn(*const ()) -> [u8; MAX_UNPARK_BYTES],
    clone_to_buffer_aligned: unsafe fn(*const ()) -> [u8; MAX_UNPARK_BYTES],
    manual_drop: unsafe fn(*const ()),
}

impl Clone for UnparkVtable {
    fn clone(&self) -> Self {
        UnparkVtable { ..*self }
    }
}

impl UnparkVtable {
    fn new<T: Unpark + Clone>() -> UnparkVtable {
        UnparkVtable {
            unpark: Self::call_unpark::<T>,
            clone_to_buffer: Self::clone_to_buffer::<T>,
            clone_to_buffer_aligned : Self::clone_to_buffer_aligned::<T>,
            manual_drop: Self::manual_drop::<T>,
        }
    }

    /// Safe if data points to T.
    unsafe fn call_unpark<T: Unpark>(data: *const ()) {
        let downcasted = read_unaligned(data as *const _ as *const T);
        downcasted.unpark();
        forget(downcasted)
    }

    /// Returns array with bytes of the cloned data. Safe if data points to T.
    /// The caller owns the new data and is responsible for dropping it with `manual_drop<T>`.
    unsafe fn clone_to_buffer<T: Clone>(data: *const ()) -> [u8; MAX_UNPARK_BYTES] {
        let downcasted = read_unaligned(data as *const _ as *const T);
        UnparkVtable::clone_to_buffer_aligned::<T>(&downcasted as *const _ as *const ())
    }

    unsafe fn clone_to_buffer_aligned<T: Clone>(data: *const ()) -> [u8; MAX_UNPARK_BYTES] {
        let downcasted = &*(data as *const _ as *const T);
        let raw = obliviate(downcasted.clone());
        forget(downcasted);
        raw
    }

    unsafe fn manual_drop<T>(data: *const ()) {
        read_unaligned(data as *const _ as *const T);
    }
}

/// Trait object that can clone `data` into an 'UnparkObj` to be put in a `Task`.
#[allow(missing_debug_implementations)]
#[derive(Copy, Clone)]
pub struct UnparkHandle<'a> {
    data: *const (),
    data_lifetime: PhantomData<&'a ()>,
    vtable: UnparkVtable,
}

impl<'a, T: Unpark + Clone> From<&'a T> for UnparkHandle<'a> {
    fn from(unpark: &T) -> UnparkHandle<'a> {
        if size_of::<T>() > MAX_UNPARK_BYTES {
            // Just panic since this is easy to catch and fix when testing.
            // Could be a compile time error when we get a const system in Rust.
            // Libraries that pass a user supplied T should do this check themselves if they want to avoid the panic.
            panic!("The size of T is {} bytes which is larger than MAX_UNPARK_BYTES={}. 
                    Wrap the unpark parameter in an Arc or use a sufficiently large MaxUnparkBytes feature.", size_of::<T>(), MAX_UNPARK_BYTES);
        }

        UnparkHandle {
            data: unpark as *const _ as *const (),
            data_lifetime: PhantomData,
            vtable: UnparkVtable::new::<T>(),
        }
    }
}

/// A custom trait object that takes ownership of the data as a slice of bytes.
pub struct UnparkObj {
    data: [u8; MAX_UNPARK_BYTES],
    vtable: UnparkVtable,
    not_sync : PhantomData<Cell<()>> // Cell is Send but not Sync, convenient.
}

impl Drop for UnparkObj {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.manual_drop)(&self.data as *const _ as *const ());
        }
    }
}

impl<'a> From<UnparkHandle<'a>> for UnparkObj {
    fn from(handle: UnparkHandle) -> UnparkObj {
        let vtable = handle.vtable;
        UnparkObj {
            data: unsafe { (vtable.clone_to_buffer_aligned)(handle.data) },
            vtable: vtable,
            not_sync: PhantomData
        }
    }
}

impl Clone for UnparkObj {
    fn clone(&self) -> Self {
        UnparkObj {
            data: unsafe { (self.vtable.clone_to_buffer)(&self.data as *const _ as *const ()) },
            ..*self
        }
    }
}

impl Unpark for UnparkObj {
    fn unpark(&self) {
        unsafe {
            (self.vtable.unpark)(&self.data as *const _ as *const ())
        }
    }
}

/// Turns the victim into raw bytes and forgets it.
/// The caller now owns the value and is responsible for dropping it with 'drop_in_place<T>'.
fn obliviate<T>(victim : T) -> [u8; MAX_UNPARK_BYTES] {
    let size = size_of::<T>();
    assert!(size < MAX_UNPARK_BYTES);
    let mut buffer = [0; MAX_UNPARK_BYTES];
    // View victim and buffer as raw bytes.
    let victim_ptr = &victim as *const _ as *const u8;
    let buffer_ptr = &mut buffer as *mut _ as *mut u8;
    // Copy from 'victim' to 'buffer' and forget 'victim'.
    // Semantically, 'buffer' now owns 'victim'.
    unsafe { ptr::copy_nonoverlapping(victim_ptr, buffer_ptr, size); }
    forget(victim);
    buffer
}

/// As implemented in core::ptr.
/// When we drop support for rust < 1.17, use core::ptr::read_unaligned instead.
unsafe fn read_unaligned<T>(src: *const T) -> T {
    let mut tmp: T = mem::uninitialized();
    ptr::copy_nonoverlapping(src as *const u8,
                            &mut tmp as *mut T as *mut u8,
                            size_of::<T>());
    tmp
}
