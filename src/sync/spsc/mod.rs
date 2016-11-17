//! A single-producer, single-consumer, futures-aware channel

use std::any::Any;
use std::error::Error;
use std::fmt;

mod bounded;
mod unbounded;
mod queue;

pub use self::bounded::{channel, Sender, Receiver};
pub use self::unbounded::{unbounded, UnboundedSender, UnboundedReceiver};

/// Error type for sending, used when the receiving end of the channel is
/// dropped
pub struct SendError<T>(T);

impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("SendError")
            .field(&"...")
            .finish()
    }
}

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "send failed because receiver is gone")
    }
}

impl<T> Error for SendError<T>
    where T: Any
{
    fn description(&self) -> &str {
        "send failed because receiver is gone"
    }
}

impl<T> SendError<T> {
    /// Returns the message that was attempted to be sent but failed.
    pub fn into_inner(self) -> T {
        self.0
    }
}

