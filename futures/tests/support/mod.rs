#![allow(dead_code)]

pub mod assert;

mod delayed;
pub use self::delayed::{delayed, Delayed};

mod run_in_background;
pub use self::run_in_background::RunInBackgroundExt;

mod counter_waker_context;
pub use self::counter_waker_context::with_counter_waker_context;

mod noop_waker_context;
pub use self::noop_waker_context::with_noop_waker_context;

mod panic_executor;

mod panic_waker_context;
pub use self::panic_waker_context::with_panic_waker_context;


// pub fn f_ok(a: i32) -> FutureResult<i32, u32> { Ok(a).into_future() }
// pub fn f_err(a: u32) -> FutureResult<i32, u32> { Err(a).into_future() }
// pub fn r_ok(a: i32) -> Result<i32, u32> { Ok(a) }
// pub fn r_err(a: u32) -> Result<i32, u32> { Err(a) }

// pub fn assert_done<T, F>(f: F, result: Result<T::Item, T::Error>)
//     where T: Future,
//           T::Item: Eq + fmt::Debug,
//           T::Error: Eq + fmt::Debug,
//           F: FnOnce() -> T,
// {
//     assert_eq!(block_on(f()), result);
// }

// pub fn assert_empty<T: Future, F: FnMut() -> T>(mut f: F)
//     where T::Error: Debug
// {
//     panic_waker_cx(|cx| {
//         assert!(f().poll(cx).unwrap().is_pending())
//     })
// }

