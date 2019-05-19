use crate::enter::Enter;
use std::time::Duration;
use futures_util::task::WakerRef;

/// Determines how long `park` will block
#[derive(Clone, Copy, Debug)]
pub enum ParkDuration {
    /// Don't block at all
    Poll,
    /// Block until explicit (possibly spurious) unpark
    Block,
    /// Block at most for given Duration; might get rounded up to some
    /// minimum "sleepable" value by `Park` implementation if timeout is
    /// too small (including zero).
    Timeout(Duration),
}

impl ParkDuration {
    /// Create a new duration specification which doesn't block longer
    /// than the passed limit.
    pub fn limit(self, max_duration: Duration) -> Self {
        match self {
            ParkDuration::Poll => ParkDuration::Poll,
            ParkDuration::Block => ParkDuration::Timeout(max_duration),
            ParkDuration::Timeout(d) => ParkDuration::Timeout(std::cmp::min(d, max_duration)),
        }
    }
}

/// Convert zero duration to `Poll`; other durations are wrapped in
/// `Timeout(..)`.
impl From<Duration> for ParkDuration {
    fn from(duration: Duration) -> Self {
        if duration == Duration::from_secs(0) {
            ParkDuration::Poll
        } else {
            ParkDuration::Timeout(duration)
        }
    }
}

/// Convert `None` to `Block`, zero durations to `Poll` and other
/// durations are wrapped in `Timeout(..)`.
impl From<Option<Duration>> for ParkDuration {
    fn from(duration: Option<Duration>) -> Self {
        match duration {
            Some(duration) => ParkDuration::from(duration),
            None => ParkDuration::Block,
        }
    }
}

/// Block the current thread.
pub trait Park {
    /// Error returned by `park`
    type Error;

    /// Get a new `Waker` associated with this `Park` instance.
    fn waker(&self) -> WakerRef<'_>;

    /// Block the current thread unless or until the token is available;
    /// `duration` determines for how long.
    ///
    /// A call to `park` does not guarantee that the thread will remain
    /// blocked forever, and callers should be prepared for this
    /// possibility. This function may wakeup spuriously for any reason.
    ///
    /// # Panics
    ///
    /// This function **should** not panic, but ultimately, panics are
    /// left as an implementation detail. Refer to the documentation for
    /// the specific `Park` implementation
    fn park(&mut self, enter: &mut Enter, duration: ParkDuration) -> Result<(), Self::Error>;
}

impl<P: Park> Park for &'_ mut P {
    type Error = P::Error;

    fn waker(&self) -> WakerRef<'_> {
        (**self).waker()
    }

    fn park(&mut self, enter: &mut Enter, duration: ParkDuration) -> Result<(), Self::Error> {
        (**self).park(enter, duration)
    }
}
