extern crate futures;

use futures::future::{killable, Killed};
use futures::sync::oneshot;

use std::thread;
use std::time::Duration;

#[test]
fn killable_works() {
    let (_tx, a_rx) = oneshot::channel::<()>();
    let (killable_rx, kill_handle) = killable(a_rx);

    let mut rx_spawn = futures::executor::spawn(killable_rx);
    kill_handle.kill();
    assert_eq!(Ok(Err(Killed)), rx_spawn.wait_future());
}

#[test]
fn killable_awakens() {
    let (_tx, a_rx) = oneshot::channel::<()>();
    let (killable_rx, kill_handle) = killable(a_rx);

    let mut rx_spawn = futures::executor::spawn(killable_rx);
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        kill_handle.kill();
    });
    assert_eq!(Ok(Err(Killed)), rx_spawn.wait_future());
}

#[test]
fn killable_resolves() {
    let (tx, a_rx) = oneshot::channel::<()>();
    let (killable_rx, _kill_handle) = killable(a_rx);

    tx.send(()).unwrap();

    let mut rx_spawn = futures::executor::spawn(killable_rx);
    assert_eq!(Ok(Ok(())), rx_spawn.wait_future());
}
