use std::sync::Arc;
use std::ops::{Deref, DerefMut};

const DEFAULT_BUF_SIZE: usize = 8 * 1024;

/// A reference counted buffer of bytes.
///
/// An `IoBuf` is a representation of a byte buffer where sub-slices of it can
/// be handed out efficiently, each with a `'static` lifetime which keeps the
/// data alive. The buffer also supports mutation but may require bytes to be
/// copied to complete the operation.
pub struct IoBuf {
    buf: Arc<Vec<u8>>,
    start: usize,
    end: usize,
}

/// An RAII object returned from `get_mut` which provides mutable access to the
/// underlying `Vec<u8>`.
pub struct IoBufMut<'a> {
    buf: &'a mut Vec<u8>,
    end: &'a mut usize,
}

impl IoBuf {
    /// Creates a new IoBuf with no data and the default capacity.
    pub fn new() -> IoBuf {
        IoBuf::with_capacity(DEFAULT_BUF_SIZE)
    }

    /// Creates a new IoBuf with `cap` capacity.
    pub fn with_capacity(cap: usize) -> IoBuf {
        IoBuf {
            buf: Arc::new(Vec::with_capacity(cap)),
            start: 0,
            end: 0,
        }
    }

    /// Changes the starting index of this window to the index specified.
    ///
    /// Returns the windows back to chain multiple calls to this method.
    ///
    /// # Panics
    ///
    /// This method will panic if `start` is out of bounds for the underlying
    /// slice or if it comes after the `end` configured in this window.
    fn set_start(&mut self, start: usize) -> &mut IoBuf {
        assert!(start < self.buf.as_ref().len());
        assert!(start <= self.end);
        self.start = start;
        self
    }

    /// Changes the end index of this window to the index specified.
    ///
    /// Returns the windows back to chain multiple calls to this method.
    ///
    /// # Panics
    ///
    /// This method will panic if `end` is out of bounds for the underlying
    /// slice or if it comes after the `end` configured in this window.
    fn set_end(&mut self, end: usize) -> &mut IoBuf {
        assert!(end < self.buf.len());
        assert!(self.start <= end);
        self.end = end;
        self
    }

    /// Returns the number of bytes contained in this `IoBuf`.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns the inner contents of this `IoBuf` as a slice.
    pub fn as_slice(&self) -> &[u8] {
        self.as_ref()
    }

    /// Splits the buffer into two at the given index.
    ///
    /// Afterwards `self` contains elements `[0, at)`, and the returned `IoBuf`
    /// contains elements `[at, len)`.
    ///
    /// This is an O(1) operation that just increases the reference count and
    /// sets a few indexes.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`
    pub fn split_off(&mut self, at: usize) -> IoBuf {
        let mut other = IoBuf { buf: self.buf.clone(), ..*self };
        let idx = self.start + at;
        other.set_start(idx);
        self.set_end(idx);
        return other
    }

    /// Splits the buffer into two at the given index.
    ///
    /// Afterwards `self` contains elements `[at, len)`, and the returned `IoBuf`
    /// contains elements `[0, at)`.
    ///
    /// This is an O(1) operation that just increases the reference count and
    /// sets a few indexes.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`
    pub fn drain_to(&mut self, at: usize) -> IoBuf {
        let mut other = IoBuf { buf: self.buf.clone(), ..*self };
        let idx = self.start + at;
        other.set_end(idx);
        self.set_start(idx);
        return other
    }

    /// Returns a mutable reference to the underlying growable buffer of bytes.
    ///
    /// If this `IoBuf` is the only instance pointing at the underlying buffer
    /// of bytes, a direct mutable reference will be returned. Otherwise the
    /// contents of this `IoBuf` will be reallocated in a fresh `Vec<u8>`
    /// allocation with the same capacity as this allocation, and that
    /// allocation will be returned.
    ///
    /// This operation **is not O(1)** as it may clone the entire contents of
    /// this buffer.
    ///
    /// The returned `IoBufMut` type implement `Deref` and `DerefMut` to
    /// `Vec<u8>` can the byte buffer can be manipulated using the standard
    /// `Vec<u8>` methods.
    pub fn get_mut(&mut self) -> IoBufMut {
        // Fast path if we can get mutable access to our own current
        // buffer.
        //
        // TODO: this should be a match or an if-let
        if Arc::get_mut(&mut self.buf).is_some() {
            let buf = Arc::get_mut(&mut self.buf).unwrap();
            buf.drain(..self.start);
            self.start = 0;
            return IoBufMut { buf: buf, end: &mut self.end }
        }

        // If we couldn't get access above then we give ourself a new buffer
        // here.
        let mut v = Vec::with_capacity(self.buf.capacity());
        v.extend_from_slice(self.as_ref());
        self.start = 0;
        self.buf = Arc::new(v);
        IoBufMut {
            buf: Arc::get_mut(&mut self.buf).unwrap(),
            end: &mut self.end,
        }
    }
}

impl AsRef<[u8]> for IoBuf {
    fn as_ref(&self) -> &[u8] {
        &self.buf[self.start..self.end]
    }
}

impl<'a> Deref for IoBufMut<'a> {
    type Target = Vec<u8>;

    fn deref(&self) -> &Vec<u8> {
        self.buf
    }
}

impl<'a> DerefMut for IoBufMut<'a> {
    fn deref_mut(&mut self) -> &mut Vec<u8> {
        self.buf
    }
}

impl<'a> Drop for IoBufMut<'a> {
    fn drop(&mut self) {
        *self.end = self.buf.len();
    }
}
