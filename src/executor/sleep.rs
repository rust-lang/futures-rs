use std::time::Duration;

/// Puts the current thread to sleep.
pub trait Sleep {
    /// Wake up handle.
    type Wakeup: Wakeup;

    /// Get a new `Wakeup` handle.
    fn wakeup(&self) -> Self::Wakeup;

    /// Put the current thread to sleep.
    fn sleep(&mut self);

    /// Put the current thread to sleep for at most `duration`.
    fn sleep_timeout(&mut self, duration: Duration);
}

/// Wake up a sleeping thread.
pub trait Wakeup: Send + Sync {
    /// Wake up the sleeping thread.
    fn wakeup(&self);
}
