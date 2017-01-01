extern crate futures;

use futures::Async;
use futures::stream;
use futures::stream::Stream;


fn run_to_panic() -> stream::Checked<stream::Empty<u32, bool>> {
    let mut empty = stream::empty();

    // It is known that `empty` return EOF forever.
    // Crash this test if that behavior of `empty` has been changed.
    assert_eq!(Ok(Async::Ready(None)), empty.poll());
    assert_eq!(Ok(Async::Ready(None)), empty.poll());
    assert_eq!(Ok(Async::Ready(None)), empty.poll());

    let mut checked = empty.checked();
    assert_eq!(Ok(Async::Ready(None)), checked.poll());
    checked
}

// Separate test `before_panic` is to make sure that next tests panics exactly
// at the last line.
#[test]
fn before_panic() {
    drop(run_to_panic());
}

#[test]
#[should_panic]
fn panics_after_eof() {
    let mut checked = run_to_panic();
    // panics here
    checked.poll().ok();
}


#[test]
fn normal_operation() {
    let mut checked = stream::iter(vec![Err(10), Ok(false)]).checked();
    assert_eq!(Err(10), checked.poll());
    assert_eq!(Ok(Async::Ready(Some(false))), checked.poll());
    assert_eq!(Ok(Async::Ready(None)), checked.poll());
}
