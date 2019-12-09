#![deny(unreachable_code)]

use futures::{try_join, executor::block_on};

#[test]
fn try_join_never_error() {
    block_on(async {
        let future1 = async { Ok::<(), !>(()) };
        let future2 = async { Ok::<(), !>(()) };
        try_join!(future1, future2)
    })
    .unwrap();
}

#[test]
fn try_join_never_ok() {
    block_on(async {
        let future1 = async { Err::<!, ()>(()) };
        let future2 = async { Err::<!, ()>(()) };
        try_join!(future1, future2)
    })
    .unwrap_err();
}
