//! Asynchronous I/O
//!
//! This crate contains the `AsyncRead` and `AsyncWrite` traits, the
//! asynchronous analogs to `std::io::{Read, Write}`. The primary difference is
//! that these traits integrate with the asynchronous task system.

#![no_std]
#![deny(missing_docs, missing_debug_implementations)]
#![doc(html_rnoot_url = "https://docs.rs/futures-io/0.2.0")]

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

if_std! {
    extern crate futures_core;
    extern crate iovec;
    extern crate std;

    use futures_core::{Poll, task};
    use std::boxed::Box;
    use std::io as StdIo;
    use std::ptr;
    use std::vec::Vec;

    // Re-export IoVec for convenience
    pub use iovec::IoVec;

    // Re-export io::Error so that users don't have to deal
    // with conflicts when `use`ing `futures::io` and `std::io`.
    pub use StdIo::Error as Error;
    pub use StdIo::ErrorKind as ErrorKind;
    pub use StdIo::Result as Result;

    /// A type used to conditionally initialize buffers passed to `AsyncRead`
    /// methods, modeled after `std`.
    #[derive(Debug)]
    pub struct Initializer(bool);

    impl Initializer {
        /// Returns a new `Initializer` which will zero out buffers.
        #[inline]
        pub fn zeroing() -> Initializer {
            Initializer(true)
        }

        /// Returns a new `Initializer` which will not zero out buffers.
        ///
        /// # Safety
        ///
        /// This method may only be called by `AsyncRead`ers which guarantee
        /// that they will not read from the buffers passed to `AsyncRead`
        /// methods, and that the return value of the method accurately reflects
        /// the number of bytes that have been written to the head of the buffer.
        #[inline]
        pub unsafe fn nop() -> Initializer {
            Initializer(false)
        }

        /// Indicates if a buffer should be initialized.
        #[inline]
        pub fn should_initialize(&self) -> bool {
            self.0
        }

        /// Initializes a buffer if necessary.
        #[inline]
        pub fn initialize(&self, buf: &mut [u8]) {
            if self.should_initialize() {
                unsafe { ptr::write_bytes(buf.as_mut_ptr(), 0, buf.len()) }
            }
        }
    }

    /// Read bytes asynchronously.
    ///
    /// This trait is analogous to the `std::io::Read` trait, but integrates
    /// with the asynchronous task system. In particular, the `poll_read`
    /// method, unlike `Read::read`, will automatically queue the current task
    /// for wakeup and return if data is not yet available, rather than blocking
    /// the calling thread.
    pub trait AsyncRead {
        /// Determines if this `AsyncRead`er can work with buffers of
        /// uninitialized memory.
        ///
        /// The default implementation returns an initializer which will zero
        /// buffers.
        ///
        /// # Safety
        ///
        /// This method is `unsafe` because and `AsyncRead`er could otherwise
        /// return a non-zeroing `Initializer` from another `AsyncRead` type
        /// without an `unsafe` block.
        #[inline]
        unsafe fn initializer(&self) -> Initializer {
            Initializer::zeroing()
        }

        /// Attempt to read from the `AsyncRead` into `buf`.
        ///
        /// On success, returns `Ok(Async::Ready(num_bytes_read))`.
        ///
        /// If no data is available for reading, the method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `cx.waker()`) to receive a notification when the object becomes
        /// readable or is closed.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Async::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_read(&mut self, cx: &mut task::Context, buf: &mut [u8])
            -> Poll<Result<usize>>;

        /// Attempt to read from the `AsyncRead` into `vec` using vectored
        /// IO operations.
        ///
        /// This method is similar to `poll_read`, but allows data to be read
        /// into multiple buffers using a single operation.
        ///
        /// On success, returns `Ok(Async::Ready(num_bytes_read))`.
        ///
        /// If no data is available for reading, the method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `cx.waker()`) to receive a notification when the object becomes
        /// readable or is closed.
        /// By default, this method delegates to using `poll_read` on the first
        /// buffer in `vec`. Objects which support vectored IO should override
        /// this method.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Async::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_vectored_read(&mut self, cx: &mut task::Context, vec: &mut [&mut IoVec])
            -> Poll<Result<usize>>
        {
            if let Some(ref mut first_iovec) = vec.get_mut(0) {
                self.poll_read(cx, first_iovec)
            } else {
                // `vec` is empty.
                Poll::Ready(Ok(0))
            }
        }
    }

    /// Write bytes asynchronously.
    ///
    /// This trait is analogous to the `std::io::Write` trait, but integrates
    /// with the asynchronous task system. In particular, the `poll_write`
    /// method, unlike `Write::write`, will automatically queue the current task
    /// for wakeup and return if data is not yet available, rather than blocking
    /// the calling thread.
    pub trait AsyncWrite {
        /// Attempt to write bytes from `buf` into the object.
        ///
        /// On success, returns `Ok(Async::Ready(num_bytes_written))`.
        ///
        /// If the object is not ready for writing, the method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `cx.waker()`) to receive a notification when the object becomes
        /// readable or is closed.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Async::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_write(&mut self, cx: &mut task::Context, buf: &[u8])
            -> Poll<Result<usize>>;

        /// Attempt to write bytes from `vec` into the object using vectored
        /// IO operations.
        ///
        /// This method is similar to `poll_write`, but allows data from multiple buffers to be written
        /// using a single operation.
        ///
        /// On success, returns `Ok(Async::Ready(num_bytes_written))`.
        ///
        /// If the object is not ready for writing, the method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `cx.waker()`) to receive a notification when the object becomes
        /// readable or is closed.
        ///
        /// By default, this method delegates to using `poll_write` on the first
        /// buffer in `vec`. Objects which support vectored IO should override
        /// this method.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Async::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_vectored_write(&mut self, cx: &mut task::Context, vec: &[&IoVec])
            -> Poll<Result<usize>>
        {
            if let Some(ref first_iovec) = vec.get(0) {
                self.poll_write(cx, &*first_iovec)
            } else {
                // `vec` is empty.
                Poll::Ready(Ok(0))
            }
        }

        /// Attempt to flush the object, ensuring that any buffered data reach
        /// their destination.
        ///
        /// On success, returns `Ok(Async::Ready(()))`.
        ///
        /// If flushing cannot immediately complete, this method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `cx.waker()`) to receive a notification when the object can make
        /// progress towards flushing.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Async::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<Result<()>>;

        /// Attempt to close the object.
        ///
        /// On success, returns `Ok(Async::Ready(()))`.
        ///
        /// If closing cannot immediately complete, this function returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `cx.waker()`) to receive a notification when the object can make
        /// progress towards closing.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Async::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_close(&mut self, cx: &mut task::Context) -> Poll<Result<()>>;
    }

    macro_rules! deref_async_read {
        () => {
            unsafe fn initializer(&self) -> Initializer {
                (**self).initializer()
            }

            fn poll_read(&mut self, cx: &mut task::Context, buf: &mut [u8])
                -> Poll<Result<usize>>
            {
                (**self).poll_read(cx, buf)
            }

            fn poll_vectored_read(&mut self, cx: &mut task::Context, vec: &mut [&mut IoVec])
                -> Poll<Result<usize>>
            {
                (**self).poll_vectored_read(cx, vec)
            }
        }
    }

    impl<T: ?Sized + AsyncRead> AsyncRead for Box<T> {
        deref_async_read!();
    }

    impl<'a, T: ?Sized + AsyncRead> AsyncRead for &'a mut T {
        deref_async_read!();
    }

    /// `unsafe` because the `StdIo::Read` type must not access the buffer
    /// before reading data into it.
    macro_rules! unsafe_delegate_async_read_to_stdio {
        () => {
            unsafe fn initializer(&self) -> Initializer {
                Initializer::nop()
            }

            fn poll_read(&mut self, _: &mut task::Context, buf: &mut [u8])
                -> Poll<Result<usize>>
            {
                Poll::Ready(StdIo::Read::read(self, buf))
            }
        }
    }

    impl<'a> AsyncRead for &'a [u8] {
        unsafe_delegate_async_read_to_stdio!();
    }

    impl AsyncRead for StdIo::Repeat {
        unsafe_delegate_async_read_to_stdio!();
    }

    impl<T: AsRef<[u8]>> AsyncRead for StdIo::Cursor<T> {
        unsafe_delegate_async_read_to_stdio!();
    }

    macro_rules! deref_async_write {
        () => {
            fn poll_write(&mut self, cx: &mut task::Context, buf: &[u8])
                -> Poll<Result<usize>>
            {
                (**self).poll_write(cx, buf)
            }

            fn poll_vectored_write(&mut self, cx: &mut task::Context, vec: &[&IoVec])
                -> Poll<Result<usize>>
            {
                (**self).poll_vectored_write(cx, vec)
            }

            fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<Result<()>> {
                (**self).poll_flush(cx)
            }

            fn poll_close(&mut self, cx: &mut task::Context) -> Poll<Result<()>> {
                (**self).poll_close(cx)
            }
        }
    }

    impl<T: ?Sized + AsyncWrite> AsyncWrite for Box<T> {
        deref_async_write!();
    }

    impl<'a, T: ?Sized + AsyncWrite> AsyncWrite for &'a mut T {
        deref_async_write!();
    }

    macro_rules! delegate_async_write_to_stdio {
        () => {
            fn poll_write(&mut self, _: &mut task::Context, buf: &[u8])
                -> Poll<Result<usize>>
            {
                Poll::Ready(StdIo::Write::write(self, buf))
            }

            fn poll_flush(&mut self, _: &mut task::Context) -> Poll<Result<()>> {
                Poll::Ready(StdIo::Write::flush(self))
            }

            fn poll_close(&mut self, cx: &mut task::Context) -> Poll<Result<()>> {
                self.poll_flush(cx)
            }
        }
    }

    impl<'a> AsyncWrite for StdIo::Cursor<&'a mut [u8]> {
        delegate_async_write_to_stdio!();
    }

    impl AsyncWrite for StdIo::Cursor<Vec<u8>> {
        delegate_async_write_to_stdio!();
    }

    impl AsyncWrite for StdIo::Cursor<Box<[u8]>> {
        delegate_async_write_to_stdio!();
    }

    impl AsyncWrite for StdIo::Sink {
        delegate_async_write_to_stdio!();
    }
}
