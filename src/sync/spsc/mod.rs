//! A single-producer, single-consumer, futures-aware channel

mod bounded;
mod unbounded;
mod queue;

pub use self::bounded::{channel, Sender, Receiver, SendError};
pub use self::unbounded::{unbounded, UnboundedSender, UnboundedReceiver};
pub use self::unbounded::UnboundedSendError;
