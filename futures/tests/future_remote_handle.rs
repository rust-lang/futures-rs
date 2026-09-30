use futures::executor::block_on;
use futures::future::{self, FutureExt};

#[test]
#[should_panic(expected = "RemoteHandle polled after Remote was dropped")]
fn remote_handle_panics_with_message_when_remote_dropped() {
    block_on(async {
        let f = future::pending::<()>();
        let (remote, remote_handle) = f.remote_handle();
        // Poll `remote` once, then drop it via `select` completing on the other branch.
        let _ = future::select(remote, future::ready(())).await;
        remote_handle.await
    });
}

#[test]
#[should_panic(expected = "RemoteHandle polled after Remote was dropped")]
fn remote_handle_panics_with_message_when_remote_dropped_unpolled() {
    block_on(async {
        let f = future::pending::<()>();
        let (remote, remote_handle) = f.remote_handle();
        drop(remote);
        remote_handle.await
    });
}
