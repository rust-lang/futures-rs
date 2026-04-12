use crate::enter::Enter;
use futures_util::task::WakerRef;
use std::task::Poll;
use std::time::Duration;

/// Determines how long [`Park::park`] will block
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum ParkDuration {
    /// Don't block at all
    Poll,
    /// Block at most for given [`Duration`]; might get rounded up to some
    /// minimum "sleepable" value by [`Park`] implementation if timeout is
    /// too small (including zero).
    Timeout(Duration),
    /// Block until explicit (possibly spurious) unpark
    Block,
}

impl ParkDuration {
    /// Create a new duration specification which doesn't block longer
    /// than the passed limit.
    pub fn limit(self, max_duration: Duration) -> Self {
        std::cmp::min(self, ParkDuration::Timeout(max_duration))
    }
}

/// Convert zero duration to [`ParkDuration::Poll`]; other durations are
/// wrapped in [`ParkDuration::Timeout`].
impl From<Duration> for ParkDuration {
    fn from(duration: Duration) -> Self {
        if duration == Duration::from_secs(0) {
            ParkDuration::Poll
        } else {
            ParkDuration::Timeout(duration)
        }
    }
}

/// Convert [`None`] to [`ParkDuration::Block`], zero durations to
/// [`ParkDuration::Poll`] and other durations are wrapped in
/// [`ParkDuration::Timeout`].
impl From<Option<Duration>> for ParkDuration {
    fn from(duration: Option<Duration>) -> Self {
        match duration {
            Some(duration) => ParkDuration::from(duration),
            None => ParkDuration::Block,
        }
    }
}

/// Block the current thread until some event occured.
///
/// The basic idea is that a `Park` implementation sleeps until the
/// waker is triggered; but before it sleeps it could also check
/// IO events and limit the duration to the next timeout event.
///
/// A timer handling `Park` implementation could forward the actual
/// parking to a nested `Park` member (which could be either an IO
/// event handler or a simple thread sleep).
///
/// Each `Park` instance should have an internal "woken" flag; it
/// is set when `Park::waker().wake()` (or similar) is called, and
/// reset when [`Park::park`] returns.
pub trait Park {
    /// Get the [`Waker`] associated with this [`Park`] instance.
    ///
    /// Waking this must interrupt a pending `park` call or prevent
    /// the next `park` call from blocking.
    ///
    /// [`Waker`]: std::task::Waker
    fn waker(&self) -> WakerRef<'_>;

    /// Block the current thread unless "woken"; `duration` determines
    /// for how long.
    ///
    /// A call to `park` does not guarantee that the thread will remain
    /// blocked forever (even when the waker isn't triggered), and
    /// callers should be prepared for this possibility. This function
    /// may wakeup spuriously for any reason.
    ///
    /// # Panics
    ///
    /// This function **should** not panic, but ultimately, panics are
    /// left as an implementation detail. Refer to the documentation for
    /// the specific [Park] implementation.
    ///
    fn park(&mut self, enter: &mut Enter, duration: ParkDuration);
}

/// Non-blocking poll for [Park]
///
/// Used for [`LocalPool::try_run_one`] and
/// [`LocalPool::run_until_stalled`]; those are not really useful, as
/// you can only poll them in a busy loop - there is no interface to
/// register another [`Waker`] to be triggered once progress could be
/// made.
///
/// [`LocalPool::try_run_one`]: super::LocalPool::try_run_one
/// [`LocalPool::run_until_stalled`]: super::LocalPool::run_until_stalled
/// [`Waker`]: std::task::Waker
pub trait ParkPoll: Park {
    /// Similar to calling [`Park::park(ParkDuration::Poll)`][Park::park].
    ///
    /// # Return value
    ///
    /// Similar to [`Park::park`] this resets the "woken" flag.
    ///
    /// Returns [`Poll::Ready`] when the "woken" flag was set,
    /// otherwise return [`Poll::Pending`].
    ///
    /// The "woken" flag might be set by some internal additional
    /// handling (e.g. checking if a timer fired) before it gets reset;
    /// this too should result in [`Poll::Ready`].
    ///
    /// A [`Poll::Pending`] result indicates that there currently is
    /// nothing more to do, while [`Poll::Ready`] indicates that some
    /// task should be able to make progress.
    fn poll(&mut self, enter: &mut Enter) -> Poll<()>;
}

impl<P: Park> Park for &'_ mut P {
    fn waker(&self) -> WakerRef<'_> {
        (**self).waker()
    }

    fn park(&mut self, enter: &mut Enter, duration: ParkDuration) {
        (**self).park(enter, duration)
    }
}

impl<P: ParkPoll> ParkPoll for &'_ mut P {
    fn poll(&mut self, enter: &mut Enter) -> Poll<()> {
        (**self).poll(enter)
    }
}

#[cfg(test)]
mod tests {
    use super::ParkDuration;
    use std::time::Duration;

    #[test]
    fn duration_order() {
        assert!(ParkDuration::Poll < ParkDuration::Block);
        assert!(ParkDuration::Poll < ParkDuration::Timeout(Duration::from_millis(100)));
        assert!(
            ParkDuration::Timeout(Duration::from_millis(100))
                < ParkDuration::Timeout(Duration::from_millis(200))
        );
        assert!(ParkDuration::Timeout(Duration::from_millis(200)) < ParkDuration::Block);
    }
}
