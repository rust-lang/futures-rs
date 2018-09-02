use std::any::Any;
use std::error::Error;
use std::fmt;

/// The error type for [`Sender`s](Sender) used as `Sink`s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendError {
    pub(super) kind: SendErrorKind,
}

/// The error type returned from [`try_send`](Sender::try_send).
#[derive(Clone, PartialEq, Eq)]
pub struct TrySendError<T> {
    pub(super) err: SendError,
    pub(super) val: T,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SendErrorKind {
    Full,
    Disconnected,
}

impl fmt::Display for SendError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.is_full() {
            write!(fmt, "send failed because channel is full")
        } else {
            write!(fmt, "send failed because receiver is gone")
        }
    }
}

impl Error for SendError {
    fn description(&self) -> &str {
        if self.is_full() {
            "send failed because channel is full"
        } else {
            "send failed because receiver is gone"
        }
    }
}

impl SendError {
    /// Returns true if this error is a result of the channel being full.
    pub fn is_full(&self) -> bool {
        match self.kind {
            SendErrorKind::Full => true,
            _ => false,
        }
    }

    /// Returns true if this error is a result of the receiver being dropped.
    pub fn is_disconnected(&self) -> bool {
        match self.kind {
            SendErrorKind::Disconnected => true,
            _ => false,
        }
    }
}

impl<T> fmt::Debug for TrySendError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("TrySendError")
            .field("kind", &self.err.kind)
            .finish()
    }
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.is_full() {
            write!(fmt, "send failed because channel is full")
        } else {
            write!(fmt, "send failed because receiver is gone")
        }
    }
}

impl<T: Any> Error for TrySendError<T> {
    fn description(&self) -> &str {
        if self.is_full() {
            "send failed because channel is full"
        } else {
            "send failed because receiver is gone"
        }
    }
}

impl<T> TrySendError<T> {
    /// Returns true if this error is a result of the channel being full.
    pub fn is_full(&self) -> bool {
        self.err.is_full()
    }

    /// Returns true if this error is a result of the receiver being dropped.
    pub fn is_disconnected(&self) -> bool {
        self.err.is_disconnected()
    }

    /// Returns the message that was attempted to be sent but failed.
    pub fn into_inner(self) -> T {
        self.val
    }

    /// Drops the message and converts into a `SendError`.
    pub fn into_send_error(self) -> SendError {
        self.err
    }
}
