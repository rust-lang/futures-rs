use std::sync::atomic::{AtomicUsize, Ordering};

use cell::AtomicCell;

/// A slot in memory intended to represent the communication channel between one
/// producer and one consumer.
///
/// Each slot contains space for a piece of data, `T`, and space for callbacks.
/// The callbacks can be run when the data is either full or empty.
pub struct Slot<T> {
    state: AtomicUsize,
    slot: AtomicCell<Option<T>>,
    on_full: AtomicCell<Option<Box<FnBox<T>>>>,
    on_empty: AtomicCell<Option<Box<FnBox<T>>>>,
}

/// Error value returned from erroneous calls to `try_produce`.
#[derive(Debug, PartialEq)]
pub struct TryProduceError<T>(T);

/// Error value returned from erroneous calls to `try_consume`.
#[derive(Debug, PartialEq)]
pub struct TryConsumeError(());

/// Error value returned from erroneous calls to `on_full`.
#[derive(Debug, PartialEq)]
pub struct OnFullError(());

/// Error value returned from erroneous calls to `on_empty`.
#[derive(Debug, PartialEq)]
pub struct OnEmptyError(());

fn _is_send<T: Send>() {}
fn _is_sync<T: Send>() {}

fn _assert() {
    _is_send::<Slot<i32>>();
    _is_sync::<Slot<u32>>();
}

const DATA: usize = 1 << 0;
const ON_FULL: usize = 1 << 1;
const ON_EMPTY: usize = 1 << 2;

impl<T: 'static> Slot<T> {
    pub fn new(val: Option<T>) -> Slot<T> {
        Slot {
            state: AtomicUsize::new(if val.is_some() {DATA} else {0}),
            slot: AtomicCell::new(val),
            on_full: AtomicCell::new(None),
            on_empty: AtomicCell::new(None),
        }
    }

    // PRODUCER
    pub fn try_produce(&self, t: T) -> Result<(), TryProduceError<T>> {
        let mut state = self.state.load(Ordering::SeqCst);
        assert!(state & ON_EMPTY == 0);
        if state & DATA != 0 {
            return Err(TryProduceError(t))
        }
        let mut slot = self.slot.borrow().expect("interference with consumer?");
        assert!(slot.is_none());
        *slot = Some(t);
        drop(slot);

        loop {
            assert!(state & ON_EMPTY == 0);
            let old = self.state.compare_and_swap(state,
                                                  (state | DATA) & !ON_FULL,
                                                  Ordering::SeqCst);
            if old == state {
                break
            }
            state = old;
        }
        assert!(state & ON_EMPTY == 0);
        if state & ON_FULL != 0 {
            let cb = self.on_full.borrow().expect("interference2")
                                 .take().expect("ON_FULL but no callback");
            cb.call_box(self);
        }
        Ok(())
    }

    // PRODUCER
    pub fn on_empty<F>(&self, f: F)
        where F: FnOnce(&Slot<T>) + Send + 'static
    {
        let mut state = self.state.load(Ordering::SeqCst);
        assert!(state & ON_EMPTY == 0);
        if state & DATA == 0 {
            return f(self)
        }
        assert!(state & ON_FULL == 0);
        let mut slot = self.on_empty.borrow().expect("on_empty interference");
        assert!(slot.is_none());
        *slot = Some(Box::new(f));
        drop(slot);

        loop {
            assert_eq!(state, DATA);
            let old = self.state.compare_and_swap(state,
                                                  state | ON_EMPTY,
                                                  Ordering::SeqCst);
            if old == state {
                break
            }
            state = old;

            if state & DATA == 0 {
                self.on_empty.borrow().expect("on_empty interference2")
                             .take().expect("on_empty not full?")
                             .call_box(self);
                break
            }
        }
    }

    // CONSUMER
    pub fn try_consume(&self) -> Result<T, TryConsumeError> {
        let mut state = self.state.load(Ordering::SeqCst);
        assert!(state & ON_FULL == 0);
        if state & DATA == 0 {
            return Err(TryConsumeError(()))
        }
        let mut slot = self.slot.borrow().expect("interference with producer?");
        let val = slot.take().expect("DATA but not data");
        drop(slot);

        loop {
            assert!(state & ON_FULL == 0);
            let old = self.state.compare_and_swap(state,
                                                  state & !DATA & !ON_EMPTY,
                                                  Ordering::SeqCst);
            if old == state {
                break
            }
            state = old;
        }
        assert!(state & ON_FULL == 0);
        if state & ON_EMPTY != 0 {
            let cb = self.on_empty.borrow().expect("interference3")
                                  .take().expect("ON_EMPTY but no callback");
            cb.call_box(self);
        }
        Ok(val)
    }

    // CONSUMER
    pub fn on_full<F>(&self, f: F)
        where F: FnOnce(&Slot<T>) + Send + 'static
    {
        let mut state = self.state.load(Ordering::SeqCst);
        assert!(state & ON_FULL == 0);
        if state & DATA == DATA {
            return f(self)
        }
        assert!(state & ON_EMPTY == 0);
        let mut slot = self.on_full.borrow().expect("on_full interference");
        assert!(slot.is_none());
        *slot = Some(Box::new(f));
        drop(slot);

        loop {
            assert_eq!(state, 0);
            let old = self.state.compare_and_swap(state,
                                                  state | ON_FULL,
                                                  Ordering::SeqCst);
            if old == state {
                break
            }
            state = old;

            if state & DATA == DATA {
                self.on_full.borrow().expect("on_full interference2")
                            .take().expect("on_full not full?")
                            .call_box(self);
                break
            }
        }
    }
}

impl<T> TryProduceError<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

trait FnBox<T: 'static>: Send + 'static {
    fn call_box(self: Box<Self>, other: &Slot<T>);
}

impl<T, F> FnBox<T> for F
    where F: FnOnce(&Slot<T>) + Send + 'static,
          T: 'static,
{
    fn call_box(self: Box<F>, other: &Slot<T>) {
        (*self)(other)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    use super::Slot;

    #[test]
    fn sequential() {
        let slot = Slot::new(Some(1));

        // We can consume once
        assert_eq!(slot.try_consume(), Ok(1));
        assert!(slot.try_consume().is_err());

        // Consume a production
        assert_eq!(slot.try_produce(2), Ok(()));
        assert_eq!(slot.try_consume(), Ok(2));

        // Can't produce twice
        assert_eq!(slot.try_produce(3), Ok(()));
        assert!(slot.try_produce(3).is_err());

        // on_full is run immediately if full
        let hit = Arc::new(AtomicUsize::new(0));
        let hit2 = hit.clone();
        slot.on_full(move |_s| {
            hit2.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(hit.load(Ordering::SeqCst), 1);

        // on_full can be run twice, and we can consume in the callback
        let hit2 = hit.clone();
        slot.on_full(move |s| {
            hit2.fetch_add(1, Ordering::SeqCst);
            assert_eq!(s.try_consume(), Ok(3));
        });
        assert_eq!(hit.load(Ordering::SeqCst), 2);

        // Production can't run a previous callback
        assert_eq!(slot.try_produce(4), Ok(()));
        assert_eq!(hit.load(Ordering::SeqCst), 2);
        assert_eq!(slot.try_consume(), Ok(4));

        // Productions run new callbacks
        let hit2 = hit.clone();
        slot.on_full(move |s| {
            hit2.fetch_add(1, Ordering::SeqCst);
            assert_eq!(s.try_consume(), Ok(5));
        });
        assert_eq!(slot.try_produce(5), Ok(()));
        assert_eq!(hit.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn channel() {
        const N: usize = 10000;

        struct Sender {
            slot: Arc<Slot<usize>>,
            hit: Arc<AtomicUsize>,
        }

        struct Receiver {
            slot: Arc<Slot<usize>>,
            hit: Arc<AtomicUsize>,
        }

        impl Sender {
            fn send(&self, val: usize) {
                if self.slot.try_produce(val).is_ok() {
                    return
                }
                let me = thread::current();
                self.hit.store(0, Ordering::SeqCst);
                let hit = self.hit.clone();
                self.slot.on_empty(move |_slot| {
                    hit.store(1, Ordering::SeqCst);
                    me.unpark();
                });
                while self.hit.load(Ordering::SeqCst) == 0 {
                    thread::park();
                }
                self.slot.try_produce(val).expect("can't produce after on_empty")
            }
        }

        impl Receiver {
            fn recv(&self) -> usize {
                if let Ok(i) = self.slot.try_consume() {
                    return i
                }

                let me = thread::current();
                self.hit.store(0, Ordering::SeqCst);
                let hit = self.hit.clone();
                self.slot.on_full(move |_slot| {
                    hit.store(1, Ordering::SeqCst);
                    me.unpark();
                });
                while self.hit.load(Ordering::SeqCst) == 0 {
                    thread::park();
                }
                self.slot.try_consume().expect("can't consume after on_full")
            }
        }

        let slot = Arc::new(Slot::new(None));
        let slot2 = slot.clone();

        let tx = Sender { slot: slot2, hit: Arc::new(AtomicUsize::new(0)) };
        let rx = Receiver { slot: slot, hit: Arc::new(AtomicUsize::new(0)) };

        let a = thread::spawn(move || {
            for i in 0..N {
                assert_eq!(rx.recv(), i);
            }
        });

        for i in 0..N {
            tx.send(i);
        }

        a.join().unwrap();
    }
}
