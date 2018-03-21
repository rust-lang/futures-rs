extern crate futures_core;
extern crate futures_executor;

use futures_core::{task, Async, Future, Poll};
use futures_core::never::Never;
use futures_executor::block_on;

struct SpawnsMore;

impl Future for SpawnsMore {
    type Item = ();
    type Error = Never;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<(), Never> {
        cx.spawn(SpawnsMore);
        Ok(Async::Ready(()))
    }
}

#[test]
#[should_panic]
fn block_on_cannot_spawn_more() {
    let _ = block_on(SpawnsMore);
}
