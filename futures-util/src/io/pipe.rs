use core::pin::Pin;
use core::ptr::copy_nonoverlapping;
use core::slice;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use alloc::boxed::Box;
use alloc::sync::Arc;
use futures_core::ready;
use futures_core::task::{Context, Poll, Waker};
use futures_io::{AsyncBufRead, AsyncRead, AsyncWrite, Error, ErrorKind, Result};

use crate::task::AtomicWaker;

/// Create a unidirectional bounded pipe for data transfer between asynchronous tasks.
///
/// The internal buffer size is given by `buffer`, which must be non zero. The [`PipeWriter`]
/// returned implements the [`AsyncWrite`] trait, while [`PipeReader`] implements [`AsyncRead`].
///
/// # Panics
///
/// Panics when `buffer` is zero.
#[must_use]
pub fn pipe(buffer: usize) -> (PipeWriter, PipeReader) {
    assert!(buffer != 0, "pipe buffer size must be non zero and not usize::MAX");
    // If it is `usize::MAX`, the allocation must fail anyway since Rust forbids allocations larger
    // than `isize::MAX as usize`. This counts as OOM thus no need to state explicitly.
    let len = buffer.saturating_add(1);
    let ptr = Box::into_raw(alloc::vec![0u8; len].into_boxed_slice());
    let inner = Arc::new(Shared {
        len,
        buffer: ptr.cast(),
        write_pos: AtomicUsize::new(0),
        read_pos: AtomicUsize::new(0),
        writer_waker: AtomicWaker::new(),
        reader_waker: AtomicWaker::new(),
        closed: AtomicBool::new(false),
    });
    (PipeWriter { inner: inner.clone() }, PipeReader { inner })
}

// `read_pos..write_pos` (loop around, same below) contains the buffered content.
// `write_pos..(read_pos-1)` is the empty space for further data.
// Note that index `read_pos-1` is left vacant so that `read_pos == write_pos` if and only if
// the buffer is empty.
//
// Invariants, at any time:
// 1.  `read_pos` and `buffer[read_pos..write_pos]` is owned by the read-end.
//     Read-end can increment `read_pos` in that range to transfer
//     a portion of buffer to the write-end.
// 2.  `write_pos` and `buffer[writer_pos..(read_pos-1)]` is owned by the write-end.
//     Write-end can increment `write_pos` in that range to transfer
//     a portion of buffer to the read-end.
// 3.  Read-end can only park (returning Pending) when it observed `read_pos == write_pos` after
//     registered the waker.
// 4.  Write-end can only park when it observed `write_pos == read_pos-1` after
//     registered the waker.
#[derive(Debug)]
struct Shared {
    len: usize,
    buffer: *mut u8,
    read_pos: AtomicUsize,
    write_pos: AtomicUsize,
    reader_waker: AtomicWaker,
    writer_waker: AtomicWaker,
    closed: AtomicBool,
}

unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

impl Drop for Shared {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(slice::from_raw_parts_mut(self.buffer, self.len)));
        }
    }
}

impl Shared {
    fn poll_read_ready(&self, waker: &Waker) -> Poll<Result<(usize, usize)>> {
        // Only mutable by us reader. No synchronization for load.
        let data_start = self.read_pos.load(Ordering::Relaxed);
        // "Acquire" the bytes for read.
        let mut data_end = self.write_pos.load(Ordering::Acquire);
        // Fast path.
        if data_start == data_end {
            // Implicit "Acquite" `write_pos` below.
            self.reader_waker.register(waker);
            // Double check for readiness.
            data_end = self.write_pos.load(Ordering::Acquire);
            if data_start == data_end {
                // Already "acquire"d by `reader_waker`.
                if self.closed.load(Ordering::Relaxed) {
                    return Poll::Ready(Ok((0, 0)));
                }
                return Poll::Pending;
            }
        }
        Poll::Ready(Ok((data_start, data_end)))
    }

    unsafe fn commit_read(&self, new_read_pos: usize) {
        // "Release" the bytes just read.
        self.read_pos.store(new_read_pos, Ordering::Release);
        // Implicit "Release" the `read_pos` change.
        self.writer_waker.wake();
    }

    fn poll_write_ready(&self, waker: &Waker) -> Poll<Result<(usize, usize)>> {
        // Only mutable by us writer. No synchronization for load.
        let write_start = self.write_pos.load(Ordering::Relaxed);
        // "Acquire" the bytes for write.
        let mut write_end =
            self.read_pos.load(Ordering::Acquire).checked_sub(1).unwrap_or(self.len - 1);
        if write_start == write_end {
            // Implicit "Acquite" `read_pos` below.
            self.writer_waker.register(waker);
            // Double check for writeness.
            write_end =
                self.read_pos.load(Ordering::Acquire).checked_sub(1).unwrap_or(self.len - 1);
            if write_start == write_end {
                // Already "acquire"d by `reader_waker`.
                if self.closed.load(Ordering::Relaxed) {
                    return Poll::Ready(Err(Error::new(ErrorKind::BrokenPipe, "pipe closed")));
                }
                return Poll::Pending;
            }
        }
        Poll::Ready(Ok((write_start, write_end)))
    }

    unsafe fn commit_write(&self, new_write_pos: usize) {
        // "Release" the bytes just written.
        self.write_pos.store(new_write_pos, Ordering::Release);
        // Implicit "Release" the `write_pos` change.
        self.reader_waker.wake();
    }
}

/// The write end of a bounded pipe.
///
/// This value is created by the [`pipe`] function.
#[derive(Debug)]
pub struct PipeWriter {
    inner: Arc<Shared>,
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        self.inner.closed.store(true, Ordering::Relaxed);
        // "Release" `closed`.
        self.inner.reader_waker.wake();
    }
}

impl AsyncWrite for PipeWriter {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        let inner = &*self.inner;

        let (write_start, write_end) = ready!(inner.poll_write_ready(cx.waker()))?;

        let written = if write_start <= write_end {
            let written = buf.len().min(write_end - write_start);
            // SAFETY: `buffer[write_pos..read_pos-1]` is owned by us writer.
            unsafe {
                copy_nonoverlapping(buf.as_ptr(), inner.buffer.add(write_start), written);
            }
            written
        } else {
            let written1 = buf.len().min(inner.len - write_start);
            let written2 = (buf.len() - written1).min(write_end);
            // SAFETY: `buffer[write_pos..]` and `buffer[..read_pos-1]` are owned by us writer.
            unsafe {
                copy_nonoverlapping(buf.as_ptr(), inner.buffer.add(write_start), written1);
                copy_nonoverlapping(buf.as_ptr().add(written1), inner.buffer, written2);
            }
            written1 + written2
        };

        let mut new_write_pos = write_start + written;
        if new_write_pos >= inner.len {
            new_write_pos -= inner.len;
        }

        unsafe {
            inner.commit_write(new_write_pos);
        }

        Poll::Ready(Ok(written))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// The read end of a bounded pipe.
///
/// This value is created by the [`pipe`] function.
#[derive(Debug)]
pub struct PipeReader {
    inner: Arc<Shared>,
}

impl Drop for PipeReader {
    fn drop(&mut self) {
        self.inner.closed.store(true, Ordering::Relaxed);
        // "Release" `closed`.
        self.inner.writer_waker.wake();
    }
}

impl AsyncRead for PipeReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        let inner = &*self.inner;

        let (data_start, data_end) = ready!(inner.poll_read_ready(cx.waker()))?;

        let read = if data_start <= data_end {
            let read = buf.len().min(data_end - data_start);
            // SAFETY: `buffer[read_pos..write_pos]` are owned by us reader.
            unsafe {
                copy_nonoverlapping(inner.buffer.add(data_start), buf.as_mut_ptr(), read);
            }
            read
        } else {
            let read1 = buf.len().min(inner.len - data_start);
            let read2 = (buf.len() - read1).min(data_end);
            // SAFETY: `buffer[read_pos..]` and `buffer[..write_pos]` are owned by us reader.
            unsafe {
                copy_nonoverlapping(inner.buffer.add(data_start), buf.as_mut_ptr(), read1);
                copy_nonoverlapping(inner.buffer, buf.as_mut_ptr().add(read1), read2);
            }
            read1 + read2
        };

        let mut new_read_pos = data_start + read;
        if new_read_pos >= inner.len {
            new_read_pos -= inner.len;
        }

        unsafe {
            self.inner.commit_read(new_read_pos);
        }

        Poll::Ready(Ok(read))
    }
}

impl AsyncBufRead for PipeReader {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>> {
        let inner = &*self.inner;
        let (data_start, mut data_end) = ready!(inner.poll_read_ready(cx.waker()))?;
        if data_end < data_start {
            data_end = inner.len;
        }
        // SAFETY: `buffer[read_pos..]` is owned by us reader.
        let data =
            unsafe { slice::from_raw_parts(inner.buffer.add(data_start), data_end - data_start) };
        Poll::Ready(Ok(data))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let inner = &*self.inner;
        // Only mutable by us reader. No synchronization for load.
        let data_start = inner.read_pos.load(Ordering::Relaxed);
        // Can only go forward since the last `poll_fill_buf` in the same thread.
        // Does not need to be up-to-date.
        let data_end = inner.write_pos.load(Ordering::Relaxed);

        let len = if data_start <= data_end {
            data_end - data_start
        } else {
            data_end + inner.len - data_start
        };
        assert!(amt <= len, "invalid advance");

        let mut new_read_pos = data_start + amt;
        if new_read_pos >= inner.len {
            new_read_pos -= inner.len;
        }
        unsafe {
            inner.commit_read(new_read_pos);
        }
    }
}
