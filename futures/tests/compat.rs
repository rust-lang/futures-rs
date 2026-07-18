#![cfg(feature = "compat")]
#![cfg(not(miri))] // Miri does not support epoll_create

use std::time::Instant;

use futures::{compat::Future01CompatExt, prelude::*};
use tokio::{runtime::Runtime, timer::Delay};

#[test]
fn can_use_01_futures_in_a_03_future_running_on_a_01_executor() {
    let f = Delay::new(Instant::now()).compat();

    let mut runtime = Runtime::new().unwrap();
    runtime.block_on(f.boxed().compat()).unwrap();
}
