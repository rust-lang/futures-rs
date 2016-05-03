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

pub struct Token(usize);

struct State(usize);

fn _is_send<T: Send>() {}
fn _is_sync<T: Send>() {}

fn _assert() {
    _is_send::<Slot<i32>>();
    _is_sync::<Slot<u32>>();
}

const DATA: usize = 1 << 0;
const ON_FULL: usize = 1 << 1;
const ON_EMPTY: usize = 1 << 2;
const STATE_BITS: usize = 3;
const STATE_MASK: usize = (1 << STATE_BITS) - 1;

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
        let mut state = State(self.state.load(Ordering::SeqCst));
        assert!(!state.flag(ON_EMPTY));
        if state.flag(DATA) {
            return Err(TryProduceError(t))
        }
        let mut slot = self.slot.borrow().expect("interference with consumer?");
        assert!(slot.is_none());
        *slot = Some(t);
        drop(slot);

        loop {
            assert!(!state.flag(ON_EMPTY));
            let new_state = state.set_flag(DATA, true).set_flag(ON_FULL, false);
            let old = self.state.compare_and_swap(state.0,
                                                  new_state.0,
                                                  Ordering::SeqCst);
            if old == state.0 {
                break
            }
            state.0 = old;
        }
        if state.flag(ON_FULL) {
            let cb = self.on_full.borrow().expect("interference2")
                                 .take().expect("ON_FULL but no callback");
            cb.call_box(self);
        }
        Ok(())
    }

    // PRODUCER
    pub fn on_empty<F>(&self, f: F) -> Token
        where F: FnOnce(&Slot<T>) + Send + 'static
    {
        let mut state = State(self.state.load(Ordering::SeqCst));
        assert!(!state.flag(ON_EMPTY));
        if !state.flag(DATA) {
            f(self);
            return Token(0)
        }
        assert!(!state.flag(ON_FULL));
        let mut slot = self.on_empty.borrow().expect("on_empty interference");
        assert!(slot.is_none());
        *slot = Some(Box::new(f));
        drop(slot);

        loop {
            assert!(state.flag(DATA));
            assert!(!state.flag(ON_FULL));
            assert!(!state.flag(ON_EMPTY));
            let new_state = state.set_flag(ON_EMPTY, true)
                                 .set_token(state.token() + 1);
            let old = self.state.compare_and_swap(state.0,
                                                  new_state.0,
                                                  Ordering::SeqCst);
            if old == state.0 {
                return Token(new_state.token())
            }
            state.0 = old;

            if !state.flag(DATA) {
                let cb = self.on_empty.borrow().expect("on_empty interference2")
                                      .take().expect("on_empty not empty??");
                cb.call_box(self);
                return Token(0)
            }
        }
    }

    // CONSUMER
    pub fn try_consume(&self) -> Result<T, TryConsumeError> {
        let mut state = State(self.state.load(Ordering::SeqCst));
        assert!(!state.flag(ON_FULL));
        if !state.flag(DATA) {
            return Err(TryConsumeError(()))
        }
        let mut slot = self.slot.borrow().expect("interference with producer?");
        let val = slot.take().expect("DATA but not data");
        drop(slot);

        loop {
            assert!(!state.flag(ON_FULL));
            let new_state = state.set_flag(DATA, false).set_flag(ON_EMPTY, false);
            let old = self.state.compare_and_swap(state.0,
                                                  new_state.0,
                                                  Ordering::SeqCst);
            if old == state.0 {
                break
            }
            state.0 = old;
        }
        assert!(!state.flag(ON_FULL));
        if state.flag(ON_EMPTY) {
            let cb = self.on_empty.borrow().expect("interference3")
                                  .take().expect("ON_EMPTY but no callback");
            cb.call_box(self);
        }
        Ok(val)
    }

    // CONSUMER
    pub fn on_full<F>(&self, f: F) -> Token
        where F: FnOnce(&Slot<T>) + Send + 'static
    {
        let mut state = State(self.state.load(Ordering::SeqCst));
        assert!(!state.flag(ON_FULL));
        if state.flag(DATA) {
            f(self);
            return Token(0)
        }
        assert!(!state.flag(ON_EMPTY));
        let mut slot = self.on_full.borrow().expect("on_full interference");
        assert!(slot.is_none());
        *slot = Some(Box::new(f));
        drop(slot);

        loop {
            assert!(!state.flag(DATA));
            assert!(!state.flag(ON_EMPTY));
            assert!(!state.flag(ON_FULL));
            let new_state = state.set_flag(ON_FULL, true)
                                 .set_token(state.token() + 1);
            let old = self.state.compare_and_swap(state.0,
                                                  new_state.0,
                                                  Ordering::SeqCst);
            if old == state.0 {
                return Token(new_state.token())
            }
            state.0 = old;

            if state.flag(DATA) {
                let cb = self.on_full.borrow().expect("on_full interference2")
                                      .take().expect("on_full not full??");
                cb.call_box(self);
                return Token(0)
            }
        }
    }

    pub fn cancel(&self, token: Token) {
        let token = token.0;
        if token == 0 {
            return
        }
        let mut state = State(self.state.load(Ordering::SeqCst));
        loop {
            if state.token() != token {
                return
            }
            let new_state = if state.flag(ON_FULL) {
                assert!(!state.flag(ON_EMPTY));
                state.set_flag(ON_FULL, false)
            } else if state.flag(ON_EMPTY) {
                assert!(!state.flag(ON_FULL));
                state.set_flag(ON_EMPTY, false)
            } else {
                return
            };
            let old = self.state.compare_and_swap(state.0,
                                                  new_state.0,
                                                  Ordering::SeqCst);
            if old != state.0 {
                break
            }
            state.0 = old;
        }

        if state.flag(ON_FULL) {
            let cb = self.on_full.borrow().expect("on_full interference3")
                                  .take().expect("on_full not full??");
            cb.call_box(self);
        } else {
            let cb = self.on_empty.borrow().expect("on_empty interference3")
                                  .take().expect("on_empty not empty??");
            cb.call_box(self);
        }
    }
}

impl<T> TryProduceError<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

pub trait FnBox<T: 'static>: Send + 'static {
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

impl State {
    fn flag(&self, f: usize) -> bool {
        self.0 & f != 0
    }

    fn set_flag(&self, f: usize, val: bool) -> State {
        State(if val {
            self.0 | f
        } else {
            self.0 & !f
        })
    }

    fn token(&self) -> usize {
        self.0 >> STATE_BITS
    }

    fn set_token(&self, gen: usize) -> State {
        State((gen << STATE_BITS) | (self.0 & STATE_MASK))
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

        // on empty should fire immediately for an empty slot
        let hit2 = hit.clone();
        slot.on_empty(move |_| {
            hit2.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(hit.load(Ordering::SeqCst), 4);
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

    #[test]
    fn cancel() {
        let slot = Slot::new(None);
        let hits = Arc::new(AtomicUsize::new(0));

        let add = || {
            let hits = hits.clone();
            move |_: &Slot<u32>| { hits.fetch_add(1, Ordering::SeqCst); }
        };

        // cancel on_full
        assert_eq!(hits.load(Ordering::SeqCst), 0);
        let hits2 = hits.clone();
        let token = slot.on_full(add());
        assert_eq!(hits.load(Ordering::SeqCst), 0);
        slot.cancel(token);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert!(slot.try_consume().is_err());

        // cancel on_empty
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        slot.try_produce(1).unwrap();
        let token = slot.on_empty(add());
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        slot.cancel(token);
        assert_eq!(hits.load(Ordering::SeqCst), 2);
        assert!(slot.try_produce(1).is_err());

        // cancel with no effect
        assert_eq!(hits.load(Ordering::SeqCst), 2);
        let token = slot.on_full(add());
        assert_eq!(hits.load(Ordering::SeqCst), 3);
        slot.cancel(token);
        assert_eq!(hits.load(Ordering::SeqCst), 3);
        assert!(slot.try_consume().is_ok());
        let token = slot.on_empty(add());
        assert_eq!(hits.load(Ordering::SeqCst), 4);
        slot.cancel(token);
        assert_eq!(hits.load(Ordering::SeqCst), 4);

        // cancel old ones don't count
        assert_eq!(hits.load(Ordering::SeqCst), 4);
        let token1 = slot.on_full(add());
        assert_eq!(hits.load(Ordering::SeqCst), 4);
        assert!(slot.try_produce(1).is_ok());
        assert_eq!(hits.load(Ordering::SeqCst), 5);
        assert!(slot.try_consume().is_ok());
        assert_eq!(hits.load(Ordering::SeqCst), 5);
        let token2 = slot.on_full(add());
        assert_eq!(hits.load(Ordering::SeqCst), 5);
        slot.cancel(token1);
        assert_eq!(hits.load(Ordering::SeqCst), 5);
        slot.cancel(token2);
        assert_eq!(hits.load(Ordering::SeqCst), 6);
    }
}
