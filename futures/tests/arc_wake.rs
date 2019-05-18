use futures::task::{ArcWake, Waker};
use std::sync::{Arc, Mutex};

struct CountingWaker {
    nr_wake: Mutex<i32>,
}

impl CountingWaker {
    fn new() -> CountingWaker {
        CountingWaker {
            nr_wake: Mutex::new(0),
        }
    }

    fn wakes(&self) -> i32 {
        *self.nr_wake.lock().unwrap()
    }
}

impl ArcWake for CountingWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let mut lock = arc_self.nr_wake.lock().unwrap();
        *lock += 1;
    }
}

#[test]
fn create_waker_from_arc() {
    let some_w = Arc::new(CountingWaker::new());

    let w1: Waker = ArcWake::into_waker(some_w.clone());
    assert_eq!(2, Arc::strong_count(&some_w));
    w1.wake_by_ref();
    assert_eq!(1, some_w.wakes());

    let w2 = w1.clone();
    assert_eq!(3, Arc::strong_count(&some_w));

    w2.wake_by_ref();
    assert_eq!(2, some_w.wakes());

    drop(w2);
    assert_eq!(2, Arc::strong_count(&some_w));
    drop(w1);
    assert_eq!(1, Arc::strong_count(&some_w));
}
