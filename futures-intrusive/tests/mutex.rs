#![feature(async_await, await_macro, futures_api)]

use futures::future::{Future};
use futures_core::future::{FusedFuture};
use futures_core::task::{Poll};
use futures_intrusive::sync::{LocalMutex};
use futures_test::task::{WakeCounter, panic_local_waker};
use pin_utils::pin_mut;

macro_rules! gen_mutex_tests {
    ($mod_name:ident, $mutex_type:ident) => {
        mod $mod_name {
            use super::*;

            #[test]
            fn uncontended_lock() {
                for is_fair in &[true, false] {
                    let lw = &panic_local_waker();
                    let mtx = $mutex_type::new(5, *is_fair);
                    assert_eq!(false, mtx.is_locked());

                    {
                        let mutex_fut = mtx.lock();
                        pin_mut!(mutex_fut);
                        match mutex_fut.as_mut().poll(lw) {
                            Poll::Pending => panic!("Expect mutex to get locked"),
                            Poll::Ready(mut guard) => {
                                assert_eq!(true, mtx.is_locked());
                                assert_eq!(5, *guard);
                                *guard += 7;
                                assert_eq!(12, *guard);
                            },
                        };
                        assert!(mutex_fut.as_mut().is_terminated());
                    }

                    assert_eq!(false, mtx.is_locked());

                    {
                        let mutex_fut = mtx.lock();
                        pin_mut!(mutex_fut);
                        match mutex_fut.as_mut().poll(lw) {
                            Poll::Pending => panic!("Expect mutex to get locked"),
                            Poll::Ready(guard) => {
                                assert_eq!(true, mtx.is_locked());
                                assert_eq!(12, *guard);
                            },
                        };
                    }

                    assert_eq!(false, mtx.is_locked());
                }
            }

            #[test]
            #[should_panic]
            fn poll_after_completion_should_panic() {
                for is_fair in &[true, false] {
                    let lw = &panic_local_waker();
                    let mtx = $mutex_type::new(5, *is_fair);

                    let mutex_fut = mtx.lock();
                    pin_mut!(mutex_fut);
                    let guard = match mutex_fut.as_mut().poll(lw) {
                        Poll::Pending => panic!("Expect mutex to get locked"),
                        Poll::Ready(guard) => guard,
                    };
                    assert_eq!(5, *guard);
                    assert!(mutex_fut.as_mut().is_terminated());

                    mutex_fut.poll(lw);
                }
            }

            #[test]
            fn contended_lock() {
                for is_fair in &[true, false] {
                    let wake_counter = WakeCounter::new();
                    let lw = &wake_counter.local_waker();
                    let mtx = $mutex_type::new(5, *is_fair);

                    let mutex_fut1 = mtx.lock();
                    pin_mut!(mutex_fut1);

                    // Lock the mutex
                    let mut guard1 = match mutex_fut1.poll(lw) {
                        Poll::Pending => panic!("Expect mutex to get locked"),
                        Poll::Ready(guard) => guard
                    };
                    *guard1 = 27;

                    // The second and third lock attempt must fail
                    let mutex_fut2 = mtx.lock();
                    pin_mut!(mutex_fut2);
                    assert!(mutex_fut2.as_mut().poll(lw).is_pending());
                    assert!(!mutex_fut2.as_mut().is_terminated());
                    let mutex_fut3 = mtx.lock();
                    pin_mut!(mutex_fut3);
                    assert!(mutex_fut3.as_mut().poll(lw).is_pending());
                    assert!(!mutex_fut3.as_mut().is_terminated());
                    assert_eq!(0, wake_counter.count());

                    // Unlock - mutex should be available again
                    drop(guard1);
                    assert_eq!(1, wake_counter.count());
                    let mut guard2 = match mutex_fut2.as_mut().poll(lw) {
                        Poll::Pending => panic!("Expect mutex to get locked"),
                        Poll::Ready(guard) => guard
                    };
                    assert_eq!(27, *guard2);
                    *guard2 = 72;
                    assert!(mutex_fut2.as_mut().is_terminated());
                    assert!(mutex_fut3.as_mut().poll(lw).is_pending());
                    assert!(!mutex_fut3.as_mut().is_terminated());
                    assert_eq!(1, wake_counter.count());

                    // Unlock - mutex should be available again
                    drop(guard2);
                    assert_eq!(2, wake_counter.count());
                    let guard3 = match mutex_fut3.as_mut().poll(lw) {
                        Poll::Pending => panic!("Expect mutex to get locked"),
                        Poll::Ready(guard) => guard
                    };
                    assert_eq!(72, *guard3);
                    assert!(mutex_fut3.as_mut().is_terminated());

                    drop(guard3);
                    assert_eq!(2, wake_counter.count());
                }
            }

            #[test]
            fn cancel_wait_for_mutex() {
                for is_fair in &[true, false] {
                    let wake_counter = WakeCounter::new();
                    let lw = &wake_counter.local_waker();
                    let mtx = $mutex_type::new(5, *is_fair);

                    let mutex_fut1 = mtx.lock();
                    pin_mut!(mutex_fut1);

                    // Lock the mutex
                    let mut guard1 = match mutex_fut1.poll(lw) {
                        Poll::Pending => panic!("Expect mutex to get locked"),
                        Poll::Ready(guard) => guard
                    };
                    *guard1 = 27;

                    // The second and third lock attempt must fail
                    let mut mutex_fut2 = Box::pin(mtx.lock());
                    let mut mutex_fut3 = Box::pin(mtx.lock());

                    assert!(mutex_fut2.as_mut().poll(lw).is_pending());
                    assert!(mutex_fut3.as_mut().poll(lw).is_pending());

                    // Before the mutex gets available, cancel one lock attempt
                    drop(mutex_fut2);

                    // Unlock - mutex should be available again. Mutex2 should have been notified
                    drop(guard1);
                    assert_eq!(1, wake_counter.count());

                    // Unlock - mutex should be available again
                    match mutex_fut3.as_mut().poll(lw) {
                        Poll::Pending => panic!("Expect mutex to get locked"),
                        Poll::Ready(guard) => guard
                    };
                }
            }

            #[test]
            fn unlock_next_when_notification_is_not_used() {
                for is_fair in &[true, false] {
                    let wake_counter = WakeCounter::new();
                    let lw = &wake_counter.local_waker();
                    let mtx = $mutex_type::new(5, *is_fair);

                    let mutex_fut1 = mtx.lock();
                    pin_mut!(mutex_fut1);

                    // Lock the mutex
                    let mut guard1 = match mutex_fut1.poll(lw) {
                        Poll::Pending => panic!("Expect mutex to get locked"),
                        Poll::Ready(guard) => guard
                    };
                    *guard1 = 27;

                    // The second and third lock attempt must fail
                    let mut mutex_fut2 = Box::pin(mtx.lock());
                    let mut mutex_fut3 = Box::pin(mtx.lock());

                    assert!(mutex_fut2.as_mut().poll(lw).is_pending());
                    assert!(!mutex_fut2.as_mut().is_terminated());

                    assert!(mutex_fut3.as_mut().poll(lw).is_pending());
                    assert!(!mutex_fut3.as_mut().is_terminated());
                    assert_eq!(0, wake_counter.count());

                    // Unlock - mutex should be available again. Mutex2 should have been notified
                    drop(guard1);
                    assert_eq!(1, wake_counter.count());

                    // We don't use the notification. Expect the next waiting task to be woken up
                    drop(mutex_fut2);
                    assert_eq!(2, wake_counter.count());

                    // Unlock - mutex should be available again
                    match mutex_fut3.as_mut().poll(lw) {
                        Poll::Pending => panic!("Expect mutex to get locked"),
                        Poll::Ready(guard) => guard
                    };
                }
            }

            #[test]
            fn new_waiters_on_unfair_mutex_can_acquire_future_while_one_task_is_notified() {
                let wake_counter = WakeCounter::new();
                let lw = &wake_counter.local_waker();
                let mtx = $mutex_type::new(5, false);

                let mutex_fut1 = mtx.lock();
                pin_mut!(mutex_fut1);

                // Lock the mutex
                let mut guard1 = match mutex_fut1.poll(lw) {
                    Poll::Pending => panic!("Expect mutex to get locked"),
                    Poll::Ready(guard) => guard
                };
                *guard1 = 27;

                // The second and third lock attempt must fail
                let mut mutex_fut2 = Box::pin(mtx.lock());
                let mut mutex_fut3 = Box::pin(mtx.lock());

                assert!(mutex_fut2.as_mut().poll(lw).is_pending());

                // Unlock - mutex should be available again. fut2 should have been notified
                drop(guard1);
                assert_eq!(1, wake_counter.count());

                // Lock fut3 in between. This should succeed
                let guard3 = match mutex_fut3.as_mut().poll(lw) {
                    Poll::Pending => panic!("Expect mutex to get locked"),
                    Poll::Ready(guard) => guard
                };
                // Now fut2 can't use it's notification and is still pending
                assert!(mutex_fut2.as_mut().poll(lw).is_pending());

                // When we drop fut3, the mutex should signal that it's available for fut2,
                // which needs to have re-registered
                drop(guard3);
                assert_eq!(2, wake_counter.count());
                match mutex_fut2.as_mut().poll(lw) {
                    Poll::Pending => panic!("Expect mutex to get locked"),
                    Poll::Ready(_guard) => {},
                };
            }

            #[test]
            fn waiters_on_unfair_mutex_can_acquire_future_through_repolling_if_one_task_is_notified() {
                let wake_counter = WakeCounter::new();
                let lw = &wake_counter.local_waker();
                let mtx = $mutex_type::new(5, false);

                let mutex_fut1 = mtx.lock();
                pin_mut!(mutex_fut1);

                // Lock the mutex
                let mut guard1 = match mutex_fut1.poll(lw) {
                    Poll::Pending => panic!("Expect mutex to get locked"),
                    Poll::Ready(guard) => guard
                };
                *guard1 = 27;

                // The second and third lock attempt must fail
                let mut mutex_fut2 = Box::pin(mtx.lock());
                let mut mutex_fut3 = Box::pin(mtx.lock());

                assert!(mutex_fut2.as_mut().poll(lw).is_pending());
                assert!(mutex_fut3.as_mut().poll(lw).is_pending());

                // Unlock - mutex should be available again. fut2 should have been notified
                drop(guard1);
                assert_eq!(1, wake_counter.count());

                // Lock fut3 in between. This should succeed
                let guard3 = match mutex_fut3.as_mut().poll(lw) {
                    Poll::Pending => panic!("Expect mutex to get locked"),
                    Poll::Ready(guard) => guard
                };
                // Now fut2 can't use it's notification and is still pending
                assert!(mutex_fut2.as_mut().poll(lw).is_pending());

                // When we drop fut3, the mutex should signal that it's available for fut2,
                // which needs to have re-registered
                drop(guard3);
                assert_eq!(2, wake_counter.count());
                match mutex_fut2.as_mut().poll(lw) {
                    Poll::Pending => panic!("Expect mutex to get locked"),
                    Poll::Ready(_guard) => {},
                };
            }



            #[test]
            fn new_waiters_on_fair_mutex_cant_acquire_future_while_one_task_is_notified() {
                let wake_counter = WakeCounter::new();
                let lw = &wake_counter.local_waker();
                let mtx = $mutex_type::new(5, true);

                let mutex_fut1 = mtx.lock();
                pin_mut!(mutex_fut1);

                // Lock the mutex
                let mut guard1 = match mutex_fut1.poll(lw) {
                    Poll::Pending => panic!("Expect mutex to get locked"),
                    Poll::Ready(guard) => guard
                };
                *guard1 = 27;

                // The second and third lock attempt must fail
                let mut mutex_fut2 = Box::pin(mtx.lock());
                let mut mutex_fut3 = Box::pin(mtx.lock());

                assert!(mutex_fut2.as_mut().poll(lw).is_pending());

                // Unlock - mutex should be available again. fut2 should have been notified
                drop(guard1);
                assert_eq!(1, wake_counter.count());

                // Lock fut3 in between. This should fail
                assert!(mutex_fut3.as_mut().poll(lw).is_pending());

                // fut2 should be lockable
                match mutex_fut2.as_mut().poll(lw) {
                    Poll::Pending => panic!("Expect mutex to get locked"),
                    Poll::Ready(_guard) => {},
                };

                // Now fut3 should have been signaled and be lockable
                assert_eq!(2, wake_counter.count());
                match mutex_fut3.as_mut().poll(lw) {
                    Poll::Pending => panic!("Expect mutex to get locked"),
                    Poll::Ready(_guard) => {},
                };
            }

            #[test]
            fn waiters_on_fair_mutex_cant_acquire_future_through_repolling_if_one_task_is_notified() {
                let wake_counter = WakeCounter::new();
                let lw = &wake_counter.local_waker();
                let mtx = $mutex_type::new(5, true);

                let mutex_fut1 = mtx.lock();
                pin_mut!(mutex_fut1);

                // Lock the mutex
                let mut guard1 = match mutex_fut1.poll(lw) {
                    Poll::Pending => panic!("Expect mutex to get locked"),
                    Poll::Ready(guard) => guard
                };
                *guard1 = 27;

                // The second and third lock attempt must fail
                let mut mutex_fut2 = Box::pin(mtx.lock());
                let mut mutex_fut3 = Box::pin(mtx.lock());

                assert!(mutex_fut2.as_mut().poll(lw).is_pending());
                assert!(mutex_fut3.as_mut().poll(lw).is_pending());

                // Unlock - mutex should be available again. fut2 should have been notified
                drop(guard1);
                assert_eq!(1, wake_counter.count());

                // Lock fut3 in between. This should fail, since fut2 should get the mutex first
                assert!(mutex_fut3.as_mut().poll(lw).is_pending());

                // fut2 should be lockable
                match mutex_fut2.as_mut().poll(lw) {
                    Poll::Pending => panic!("Expect mutex to get locked"),
                    Poll::Ready(_guard) => {},
                };

                // Now fut3 should be lockable
                assert_eq!(2, wake_counter.count());

                match mutex_fut3.as_mut().poll(lw) {
                    Poll::Pending => panic!("Expect mutex to get locked"),
                    Poll::Ready(_guard) => {},
                };
            }
        }
    }
}

gen_mutex_tests!(local_mutex_tests, LocalMutex);

#[cfg(feature = "std")]
mod if_std {
    use super::*;
    use futures_intrusive::sync::{Mutex};

    gen_mutex_tests!(mutex_tests, Mutex);
}