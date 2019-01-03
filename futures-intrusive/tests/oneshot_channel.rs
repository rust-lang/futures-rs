#![feature(async_await, await_macro, futures_api)]

use futures::future::{Future};
use futures_core::future::{FusedFuture};
use futures_core::task::Poll;
use futures_intrusive::channel::{LocalOneshotChannel};
use futures_test::task::{WakeCounter, panic_local_waker};
use pin_utils::pin_mut;

macro_rules! gen_oneshot_tests {
    ($mod_name:ident, $channel_type:ident) => {
        mod $mod_name {
            use super::*;

           #[test]
            fn receive_after_send() {
                let channel = $channel_type::<i32>::new();
                let lw = &panic_local_waker();

                channel.send(5).unwrap();

                let receive_fut = channel.receive();
                pin_mut!(receive_fut);
                assert!(!receive_fut.as_mut().is_terminated());

                let res = {
                    let poll_res = receive_fut.as_mut().poll(lw);
                    match poll_res {
                        Poll::Pending => panic!("future is not ready"),
                        Poll::Ready(res) => res,
                    }
                };
                match res {
                    None => panic!("Unexpected closed channel"),
                    Some(5) => {}, // OK
                    Some(_) => panic!("Unexpected value"),
                }

                assert!(receive_fut.as_mut().is_terminated());

                // A second receive attempt must yield None, since the
                // value was taken out of the channel
                let receive_fut2 = channel.receive();
                pin_mut!(receive_fut2);
                match receive_fut2.as_mut().poll(lw) {
                    Poll::Pending => panic!("future is not ready"),
                    Poll::Ready(res) => {
                        match res {
                            None => {}, // OK. Channel must be closed
                            Some(_) => panic!("Unexpected value"),
                        }
                    }
                }
            }

            #[test]
            fn send_after_receive() {
                let channel = $channel_type::<i32>::new();
                let wake_counter = WakeCounter::new();
                let lw = &wake_counter.local_waker();

                let receive_fut1 = channel.receive();
                let receive_fut2 = channel.receive();
                pin_mut!(receive_fut1);
                pin_mut!(receive_fut2);
                assert!(!receive_fut1.as_mut().is_terminated());
                assert!(!receive_fut2.as_mut().is_terminated());

                let poll_res1 = receive_fut1.as_mut().poll(lw);
                let poll_res2 = receive_fut2.as_mut().poll(lw);
                assert!(poll_res1.is_pending());
                assert!(poll_res2.is_pending());

                channel.send(5).unwrap();

                match receive_fut1.as_mut().poll(lw) {
                    Poll::Pending => panic!("future is not ready"),
                    Poll::Ready(res) => {
                        match res {
                            None => panic!("Unexpected closed channel"),
                            Some(5) => {}, // OK
                            Some(_) => panic!("Unexpected value"),
                        }
                    }
                }

                assert!(receive_fut1.as_mut().is_terminated());
                assert!(!receive_fut2.as_mut().is_terminated());

                match receive_fut2.as_mut().poll(lw) {
                    Poll::Pending => panic!("future is not ready"),
                    Poll::Ready(res) => {
                        match res {
                            None => {}, // OK. Channel must be closed
                            Some(_) => panic!("Unexpected value"),
                        }
                    }
                }

                assert!(receive_fut1.as_mut().is_terminated());
                assert!(receive_fut2.as_mut().is_terminated());
            }

            #[test]
            fn second_send_rejects_value() {
                let channel = $channel_type::<i32>::new();
                let wake_counter = WakeCounter::new();
                let lw = &wake_counter.local_waker();

                let receive_fut1 = channel.receive();
                pin_mut!(receive_fut1);
                assert!(!receive_fut1.as_mut().is_terminated());
                assert!(receive_fut1.as_mut().poll(lw).is_pending());

                // First send
                channel.send(5).unwrap();

                assert!(receive_fut1.as_mut().poll(lw).is_ready());

                // Second send
                let send_res = channel.send(7);
                match send_res {
                    Err(7) => {}, // expected
                    _ => panic!("Second second should reject"),
                }

            }

            #[test]
            fn cancel_mid_wait() {
                let channel = $channel_type::new();
                let wake_counter = WakeCounter::new();
                let lw = &wake_counter.local_waker();

                {
                    // Cancel a wait in between other waits
                    // In order to arbitrarily drop a non movable future we have to box and pin it
                    let mut poll1 = Box::pin(channel.receive());
                    let mut poll2 = Box::pin(channel.receive());
                    let mut poll3 = Box::pin(channel.receive());
                    let mut poll4 = Box::pin(channel.receive());
                    let mut poll5 = Box::pin(channel.receive());

                    assert!(poll1.as_mut().poll(lw).is_pending());
                    assert!(poll2.as_mut().poll(lw).is_pending());
                    assert!(poll3.as_mut().poll(lw).is_pending());
                    assert!(poll4.as_mut().poll(lw).is_pending());
                    assert!(poll5.as_mut().poll(lw).is_pending());
                    assert!(!poll1.is_terminated());
                    assert!(!poll2.is_terminated());
                    assert!(!poll3.is_terminated());
                    assert!(!poll4.is_terminated());
                    assert!(!poll5.is_terminated());

                    // Cancel 2 futures. Only the remaining ones should get completed
                    drop(poll2);
                    drop(poll4);

                    assert!(poll1.as_mut().poll(lw).is_pending());
                    assert!(poll3.as_mut().poll(lw).is_pending());
                    assert!(poll5.as_mut().poll(lw).is_pending());

                    assert_eq!(0, wake_counter.count());
                    channel.send(7).unwrap();
                    assert_eq!(3, wake_counter.count());

                    assert!(poll1.as_mut().poll(lw).is_ready());
                    assert!(poll3.as_mut().poll(lw).is_ready());
                    assert!(poll5.as_mut().poll(lw).is_ready());
                    assert!(poll1.is_terminated());
                    assert!(poll3.is_terminated());
                    assert!(poll5.is_terminated());
                }

                assert_eq!(3, wake_counter.count());
            }

            #[test]
            fn cancel_end_wait() {
                let channel = $channel_type::new();
                let wake_counter = WakeCounter::new();
                let lw = &wake_counter.local_waker();

                let poll1 = channel.receive();
                let poll2 = channel.receive();
                let poll3 = channel.receive();
                let poll4 = channel.receive();

                pin_mut!(poll1);
                pin_mut!(poll2);
                pin_mut!(poll3);
                pin_mut!(poll4);

                assert!(poll1.as_mut().poll(lw).is_pending());
                assert!(poll2.as_mut().poll(lw).is_pending());

                // Start polling some wait handles which get cancelled
                // before new ones are attached
                {
                    let poll5 = channel.receive();
                    let poll6 = channel.receive();
                    pin_mut!(poll5);
                    pin_mut!(poll6);
                    assert!(poll5.as_mut().poll(lw).is_pending());
                    assert!(poll6.as_mut().poll(lw).is_pending());
                }

                assert!(poll3.as_mut().poll(lw).is_pending());
                assert!(poll4.as_mut().poll(lw).is_pending());

                channel.send(99).unwrap();

                assert!(poll1.as_mut().poll(lw).is_ready());
                assert!(poll2.as_mut().poll(lw).is_ready());
                assert!(poll3.as_mut().poll(lw).is_ready());
                assert!(poll4.as_mut().poll(lw).is_ready());

                assert_eq!(4, wake_counter.count());
            }
        }
    }
}

gen_oneshot_tests!(local_oneshot_channel_tests, LocalOneshotChannel);

#[cfg(feature = "std")]
mod if_std {
    use super::*;
    use futures_intrusive::channel::OneshotChannel;

    gen_oneshot_tests!(oneshot_channel_tests, OneshotChannel);
}