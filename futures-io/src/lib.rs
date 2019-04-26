//! Asynchronous I/O
//!
//! This crate contains the `AsyncRead` and `AsyncWrite` traits, the
//! asynchronous analogs to `std::io::{Read, Write}`. The primary difference is
//! that these traits integrate with the asynchronous task system.

#![cfg_attr(not(feature = "std"), no_std)]

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]

#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.15/futures_io")]

#[cfg(feature = "std")]
mod if_std {
    use futures_core::task::{Context, Poll};
    use std::boxed::Box;
    use std::cmp;
    use std::io as StdIo;
    use std::ops::DerefMut;
    use std::pin::Pin;
    use std::ptr;

    // Re-export IoVec for convenience
    pub use iovec::IoVec;

    // Re-export some types from `std::io` so that users don't have to deal
    // with conflicts when `use`ing `futures::io` and `std::io`.
    pub use self::StdIo::Error as Error;
    pub use self::StdIo::ErrorKind as ErrorKind;
    pub use self::StdIo::Result as Result;
    pub use self::StdIo::SeekFrom as SeekFrom;

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
        /// On success, returns `Poll::Ready(Ok(num_bytes_read))`.
        ///
        /// If no data is available for reading, the method returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object becomes
        /// readable or is closed.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8])
            -> Poll<Result<usize>>;

        /// Attempt to read from the `AsyncRead` into `vec` using vectored
        /// IO operations.
        ///
        /// This method is similar to `poll_read`, but allows data to be read
        /// into multiple buffers using a single operation.
        ///
        /// On success, returns `Poll::Ready(Ok(num_bytes_read))`.
        ///
        /// If no data is available for reading, the method returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object becomes
        /// readable or is closed.
        /// By default, this method delegates to using `poll_read` on the first
        /// buffer in `vec`. Objects which support vectored IO should override
        /// this method.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_vectored_read(self: Pin<&mut Self>, cx: &mut Context<'_>, vec: &mut [&mut IoVec])
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
        /// On success, returns `Poll::Ready(Ok(num_bytes_written))`.
        ///
        /// If the object is not ready for writing, the method returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object becomes
        /// readable or is closed.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8])
            -> Poll<Result<usize>>;

        /// Attempt to write bytes from `vec` into the object using vectored
        /// IO operations.
        ///
        /// This method is similar to `poll_write`, but allows data from multiple buffers to be written
        /// using a single operation.
        ///
        /// On success, returns `Poll::Ready(Ok(num_bytes_written))`.
        ///
        /// If the object is not ready for writing, the method returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object becomes
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
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_vectored_write(self: Pin<&mut Self>, cx: &mut Context<'_>, vec: &[&IoVec])
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
        /// On success, returns `Poll::Ready(Ok(()))`.
        ///
        /// If flushing cannot immediately complete, this method returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object can make
        /// progress towards flushing.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>>;

        /// Attempt to close the object.
        ///
        /// On success, returns `Poll::Ready(Ok(()))`.
        ///
        /// If closing cannot immediately complete, this function returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object can make
        /// progress towards closing.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>>;
    }

    /// Seek bytes asynchronously.
    ///
    /// This trait is analogous to the `std::io::Seek` trait, but integrates
    /// with the asynchronous task system. In particular, the `poll_seek`
    /// method, unlike `Seek::seek`, will automatically queue the current task
    /// for wakeup and return if data is not yet available, rather than blocking
    /// the calling thread.
    pub trait AsyncSeek {
        /// Attempt to seek to an offset, in bytes, in a stream.
        ///
        /// A seek beyond the end of a stream is allowed, but behavior is defined
        /// by the implementation.
        ///
        /// If the seek operation completed successfully,
        /// this method returns the new position from the start of the stream.
        /// That position can be used later with [`SeekFrom::Start`].
        ///
        /// # Errors
        ///
        /// Seeking to a negative offset is considered an error.
        fn poll_seek(self: Pin<&mut Self>, cx: &mut Context<'_>, pos: SeekFrom)
            -> Poll<Result<u64>>;
    }

    /// Read bytes asynchronously.
    ///
    /// This trait is analogous to the `std::io::BufRead` trait, but integrates
    /// with the asynchronous task system. In particular, the `poll_fill_buf`
    /// method, unlike `BufRead::fill_buf`, will automatically queue the current task
    /// for wakeup and return if data is not yet available, rather than blocking
    /// the calling thread.
    pub trait AsyncBufRead: AsyncRead {
        /// Attempt to return the contents of the internal buffer, filling it with more data
        /// from the inner reader if it is empty.
        ///
        /// On success, returns `Poll::Ready(Ok(buf))`.
        ///
        /// If no data is available for reading, the method returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object becomes
        /// readable or is closed.
        ///
        /// This function is a lower-level call. It needs to be paired with the
        /// [`consume`] method to function properly. When calling this
        /// method, none of the contents will be "read" in the sense that later
        /// calling [`poll_read`] may return the same contents. As such, [`consume`] must
        /// be called with the number of bytes that are consumed from this buffer to
        /// ensure that the bytes are never returned twice.
        ///
        /// [`poll_read`]: AsyncRead::poll_read
        /// [`consume`]: AsyncBufRead::consume
        ///
        /// An empty buffer returned indicates that the stream has reached EOF.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_fill_buf<'a>(self: Pin<&'a mut Self>, cx: &mut Context<'_>)
            -> Poll<Result<&'a [u8]>>;

        /// Tells this buffer that `amt` bytes have been consumed from the buffer,
        /// so they should no longer be returned in calls to [`poll_read`].
        ///
        /// This function is a lower-level call. It needs to be paired with the
        /// [`poll_fill_buf`] method to function properly. This function does
        /// not perform any I/O, it simply informs this object that some amount of
        /// its buffer, returned from [`poll_fill_buf`], has been consumed and should
        /// no longer be returned. As such, this function may do odd things if
        /// [`poll_fill_buf`] isn't called before calling it.
        ///
        /// The `amt` must be `<=` the number of bytes in the buffer returned by
        /// [`poll_fill_buf`].
        ///
        /// [`poll_read`]: AsyncRead::poll_read
        /// [`poll_fill_buf`]: AsyncBufRead::poll_fill_buf
        fn consume(self: Pin<&mut Self>, amt: usize);
    }

    macro_rules! deref_async_read {
        () => {
            unsafe fn initializer(&self) -> Initializer {
                (**self).initializer()
            }

            fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8])
                -> Poll<Result<usize>>
            {
                Pin::new(&mut **self).poll_read(cx, buf)
            }

            fn poll_vectored_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, vec: &mut [&mut IoVec])
                -> Poll<Result<usize>>
            {
                Pin::new(&mut **self).poll_vectored_read(cx, vec)
            }
        }
    }

    impl<T: ?Sized + AsyncRead + Unpin> AsyncRead for Box<T> {
        deref_async_read!();
    }

    impl<T: ?Sized + AsyncRead + Unpin> AsyncRead for &mut T {
        deref_async_read!();
    }

    impl<P> AsyncRead for Pin<P>
    where
        P: DerefMut + Unpin,
        P::Target: AsyncRead,
    {
        unsafe fn initializer(&self) -> Initializer {
            (**self).initializer()
        }

        fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8])
            -> Poll<Result<usize>>
        {
            Pin::get_mut(self).as_mut().poll_read(cx, buf)
        }

        fn poll_vectored_read(self: Pin<&mut Self>, cx: &mut Context<'_>, vec: &mut [&mut IoVec])
            -> Poll<Result<usize>>
        {
            Pin::get_mut(self).as_mut().poll_vectored_read(cx, vec)
        }
    }

    /// `unsafe` because the `StdIo::Read` type must not access the buffer
    /// before reading data into it.
    macro_rules! unsafe_delegate_async_read_to_stdio {
        () => {
            unsafe fn initializer(&self) -> Initializer {
                Initializer::nop()
            }

            fn poll_read(mut self: Pin<&mut Self>, _: &mut Context<'_>, buf: &mut [u8])
                -> Poll<Result<usize>>
            {
                Poll::Ready(StdIo::Read::read(&mut *self, buf))
            }
        }
    }

    impl AsyncRead for &[u8] {
        unsafe_delegate_async_read_to_stdio!();
    }

    impl AsyncRead for StdIo::Repeat {
        unsafe_delegate_async_read_to_stdio!();
    }

    impl AsyncRead for StdIo::Empty {
        unsafe_delegate_async_read_to_stdio!();
    }

    impl<T: AsRef<[u8]> + Unpin> AsyncRead for StdIo::Cursor<T> {
        unsafe_delegate_async_read_to_stdio!();
    }

    macro_rules! deref_async_write {
        () => {
            fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8])
                -> Poll<Result<usize>>
            {
                Pin::new(&mut **self).poll_write(cx, buf)
            }

            fn poll_vectored_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, vec: &[&IoVec])
                -> Poll<Result<usize>>
            {
                Pin::new(&mut **self).poll_vectored_write(cx, vec)
            }

            fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
                Pin::new(&mut **self).poll_flush(cx)
            }

            fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
                Pin::new(&mut **self).poll_close(cx)
            }
        }
    }

    impl<T: ?Sized + AsyncWrite + Unpin> AsyncWrite for Box<T> {
        deref_async_write!();
    }

    impl<T: ?Sized + AsyncWrite + Unpin> AsyncWrite for &mut T {
        deref_async_write!();
    }

    impl<P> AsyncWrite for Pin<P>
    where
        P: DerefMut + Unpin,
        P::Target: AsyncWrite,
    {
        fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8])
            -> Poll<Result<usize>>
        {
            Pin::get_mut(self).as_mut().poll_write(cx, buf)
        }

        fn poll_vectored_write(self: Pin<&mut Self>, cx: &mut Context<'_>, vec: &[&IoVec])
            -> Poll<Result<usize>>
        {
            Pin::get_mut(self).as_mut().poll_vectored_write(cx, vec)
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
            Pin::get_mut(self).as_mut().poll_flush(cx)
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
            Pin::get_mut(self).as_mut().poll_close(cx)
        }
    }

    macro_rules! delegate_async_write_to_stdio {
        () => {
            fn poll_write(mut self: Pin<&mut Self>, _: &mut Context<'_>, buf: &[u8])
                -> Poll<Result<usize>>
            {
                Poll::Ready(StdIo::Write::write(&mut *self, buf))
            }

            fn poll_flush(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
                Poll::Ready(StdIo::Write::flush(&mut *self))
            }

            fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
                self.poll_flush(cx)
            }
        }
    }

    impl<T: AsMut<[u8]> + Unpin> AsyncWrite for StdIo::Cursor<T> {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize>> {
            let position = self.position();
            let result = {
                let out = (&mut *self).get_mut().as_mut();
                let pos = cmp::min(out.len() as u64, position) as usize;
                StdIo::Write::write(&mut &mut out[pos..], buf)
            };
            if let Ok(offset) = result {
                self.get_mut().set_position(position + offset as u64);
            }
            Poll::Ready(result)
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
            Poll::Ready(StdIo::Write::flush(&mut self.get_mut().get_mut().as_mut()))
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
            self.poll_flush(cx)
        }
    }

    impl AsyncWrite for Vec<u8> {
        delegate_async_write_to_stdio!();
    }

    impl AsyncWrite for StdIo::Sink {
        delegate_async_write_to_stdio!();
    }

    macro_rules! deref_async_seek {
        () => {
            fn poll_seek(mut self: Pin<&mut Self>, cx: &mut Context<'_>, pos: SeekFrom)
                -> Poll<Result<u64>>
            {
                Pin::new(&mut **self).poll_seek(cx, pos)
            }
        }
    }

    impl<T: ?Sized + AsyncSeek + Unpin> AsyncSeek for Box<T> {
        deref_async_seek!();
    }

    impl<T: ?Sized + AsyncSeek + Unpin> AsyncSeek for &mut T {
        deref_async_seek!();
    }


    impl<P> AsyncSeek for Pin<P>
    where
        P: DerefMut + Unpin,
        P::Target: AsyncSeek,
    {
        fn poll_seek(self: Pin<&mut Self>, cx: &mut Context<'_>, pos: SeekFrom)
            -> Poll<Result<u64>>
        {
            self.get_mut().as_mut().poll_seek(cx, pos)
        }
    }

    macro_rules! delegate_async_seek_to_stdio {
        () => {
            fn poll_seek(mut self: Pin<&mut Self>, _: &mut Context<'_>, pos: SeekFrom)
            -> Poll<Result<u64>>
            {
                Poll::Ready(StdIo::Seek::seek(&mut *self, pos))
            }
        }
    }

    impl<T: AsRef<[u8]> + Unpin> AsyncSeek for StdIo::Cursor<T> {
        delegate_async_seek_to_stdio!();
    }

    macro_rules! deref_async_buf_read {
        () => {
            fn poll_fill_buf<'a>(self: Pin<&'a mut Self>, cx: &mut Context<'_>)
                -> Poll<Result<&'a [u8]>>
            {
                Pin::new(&mut **self.get_mut()).poll_fill_buf(cx)
            }

            fn consume(self: Pin<&mut Self>, amt: usize) {
                Pin::new(&mut **self.get_mut()).consume(amt)
            }
        }
    }

    impl<T: ?Sized + AsyncBufRead + Unpin> AsyncBufRead for Box<T> {
        deref_async_buf_read!();
    }

    impl<T: ?Sized + AsyncBufRead + Unpin> AsyncBufRead for &mut T {
        deref_async_buf_read!();
    }

    impl<P> AsyncBufRead for Pin<P>
    where
        P: DerefMut + Unpin,
        P::Target: AsyncBufRead,
    {
        fn poll_fill_buf<'a>(self: Pin<&'a mut Self>, cx: &mut Context<'_>)
            -> Poll<Result<&'a [u8]>>
        {
            self.get_mut().as_mut().poll_fill_buf(cx)
        }

        fn consume(self: Pin<&mut Self>, amt: usize) {
            self.get_mut().as_mut().consume(amt)
        }
    }

    macro_rules! delegate_async_buf_read_to_stdio {
        () => {
            fn poll_fill_buf<'a>(self: Pin<&'a mut Self>, _: &mut Context<'_>)
                -> Poll<Result<&'a [u8]>>
            {
                Poll::Ready(StdIo::BufRead::fill_buf(self.get_mut()))
            }

            fn consume(self: Pin<&mut Self>, amt: usize) {
                StdIo::BufRead::consume(self.get_mut(), amt)
            }
        }
    }

    impl AsyncBufRead for &[u8] {
        delegate_async_buf_read_to_stdio!();
    }

    impl AsyncBufRead for StdIo::Empty {
        delegate_async_buf_read_to_stdio!();
    }

    impl<T: AsRef<[u8]> + Unpin> AsyncBufRead for StdIo::Cursor<T> {
        delegate_async_buf_read_to_stdio!();
    }
}

#[cfg(feature = "std")]
pub use self::if_std::*;
