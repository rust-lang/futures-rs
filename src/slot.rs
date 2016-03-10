use cell::AtomicCell;

/// A slot in memory intended to represent the communication channel between one
/// producer and one consumer.
///
/// Each slot contains space for a piece of data, `T`, and space for callbacks.
/// The callbacks can be run when the data is either full or empty.
pub struct Slot<T> {
    slot: AtomicCell<Option<T>>,
    callback: AtomicCell<Option<Box<FnBox<T>>>>,
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

impl<T: 'static> Slot<T> {
    pub fn new(val: Option<T>) -> Slot<T> {
        Slot {
            slot: AtomicCell::new(val),
            callback: AtomicCell::new(None),
        }
    }

    // PRODUCER
    pub fn try_produce(&self, t: T) -> Result<(), TryProduceError<T>> {
        let mut slot = match self.slot.borrow() {
            Some(borrow) => borrow,
            None => {
                println!("producer interference");
                return Err(TryProduceError(t))
            }
        };
        if slot.is_some() {
            println!("producer already produced");
            return Err(TryProduceError(t))
        }
        *slot = Some(t);
        println!("producer producing");
        drop(slot);
        let callback = self.callback.borrow().and_then(|mut cb| cb.take());
        if let Some(callback) = callback {
            println!("producer running callback");
            callback.call_box(self);
        }
        Ok(())
    }

    // PRODUCER
    pub fn on_empty<F>(&self, f: F) -> Result<(), OnEmptyError>
        where F: FnOnce(&Slot<T>) + Send + 'static
    {
        if self.slot.borrow().map(|s| s.is_none()) == Some(true) {
            println!("on_empty immediate");
            return Ok(f(self))
        }
        match self.callback.borrow() {
            Some(mut callback) => {
                if callback.is_some() {
                    println!("on_empty already full");
                    drop(callback);
                    return Ok(f(self))
                } else {
                    *callback = Some(Box::new(f));
                    println!("on_empty callback registered");
                }
            }
            None => {
                println!("on_empty interference");
                return Ok(f(self))
            }
        }

        if self.slot.borrow().map(|s| s.is_none()) == Some(true) {
            println!("on_empty looking for callback");
            let cb = self.callback.borrow().and_then(|mut cb| cb.take());
            if let Some(cb) = cb {
                println!("on_empty running callback");
                cb.call_box(self);
            }
        }
        Ok(())
    }

    // CONSUMER
    pub fn try_consume(&self) -> Result<T, TryConsumeError> {
        let mut slot = match self.slot.borrow() {
            Some(borrow) => borrow,
            None => {
                println!("consumer interference");
                return Err(TryConsumeError(()))
            }
        };
        let val = match slot.take() {
            Some(t) => t,
            None => {
                println!("consumer nothing available");
                return Err(TryConsumeError(()))
            }
        };
        println!("consumer consumed");
        drop(slot);
        let callback = self.callback.borrow().and_then(|mut cb| cb.take());
        if let Some(callback) = callback {
            println!("consumer running callback");
            callback.call_box(self);
        }
        Ok(val)
    }

    // CONSUMER
    pub fn on_full<F>(&self, f: F) -> Result<(), OnFullError>
        where F: FnOnce(&Slot<T>) + Send + 'static
    {
        if self.slot.borrow().map(|s| s.is_some()) == Some(true) {
            println!("on full immediate");
            return Ok(f(self))
        }

        match self.callback.borrow() {
            Some(mut callback) => {
                if callback.is_some() {
                    println!("on full already full");
                    drop(callback);
                    return Ok(f(self))
                } else {
                    *callback = Some(Box::new(f));
                    println!("on_full callback registered");
                }
            }
            None => {
                println!("on full interference");
                return Ok(f(self))
            }
        }

        if self.slot.borrow().map(|s| s.is_some()) == Some(true) {
            println!("on full looking for callback");
            let cb = self.callback.borrow().and_then(|mut cb| cb.take());
            if let Some(cb) = cb {
                println!("on full callback");
                cb.call_box(self);
            }
        }
        Ok(())
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
        assert!(slot.on_full(move |_s| {
            hit2.fetch_add(1, Ordering::SeqCst);
        }).is_ok());
        assert_eq!(hit.load(Ordering::SeqCst), 1);

        // on_full can be run twice, and we can consume in the callback
        let hit2 = hit.clone();
        assert!(slot.on_full(move |s| {
            hit2.fetch_add(1, Ordering::SeqCst);
            assert_eq!(s.try_consume(), Ok(3));
        }).is_ok());
        assert_eq!(hit.load(Ordering::SeqCst), 2);

        // Production can't run a previous callback
        assert_eq!(slot.try_produce(4), Ok(()));
        assert_eq!(hit.load(Ordering::SeqCst), 2);
        assert_eq!(slot.try_consume(), Ok(4));

        // Productions run new callbacks
        let hit2 = hit.clone();
        assert!(slot.on_full(move |s| {
            hit2.fetch_add(1, Ordering::SeqCst);
            assert_eq!(s.try_consume(), Ok(5));
        }).is_ok());
        assert_eq!(slot.try_produce(5), Ok(()));
        assert_eq!(hit.load(Ordering::SeqCst), 3);

        // callbacks don't run when consuming
        assert!(slot.on_full(|_s| {}).is_ok());
        assert!(slot.try_consume().is_err());
        assert!(slot.try_produce(4).is_ok());
    }

    #[test]
    fn channel() {
        const N: usize = 1000;

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
                assert!(self.slot.on_empty(move |_slot| {
                    hit.store(1, Ordering::SeqCst);
                    me.unpark();
                }).is_ok());
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
                assert!(self.slot.on_full(move |_slot| {
                    hit.store(1, Ordering::SeqCst);
                    me.unpark();
                }).is_ok());
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
