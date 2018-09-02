use std::error::Error;
use std::fmt;

/// The error type returned from [`try_next`](Receiver::try_next).
pub struct TryRecvError {
    pub(super) _inner: (),
}

impl fmt::Debug for TryRecvError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("TryRecvError")
            .finish()
    }
}

impl fmt::Display for TryRecvError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str(self.description())
    }
}

impl Error for TryRecvError {
    fn description(&self) -> &str {
        "receiver channel is empty"
    }
}
