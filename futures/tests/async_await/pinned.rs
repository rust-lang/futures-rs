use futures::stable::{block_on_stable, StableFuture};
use futures::prelude::*;

#[async]
fn foo() -> Result<i32, i32> {
    Ok(1)
}

#[async]
fn bar(x: &i32) -> Result<i32, i32> {
    Ok(*x)
}

#[async]
fn baz(x: i32) -> Result<i32, i32> {
    await!(bar(&x))
}

#[async_move]
fn qux(x: i32) -> Result<i32, i32> {
    await!(baz(x).pin())
}

// This test is to check that the internal __await macro generated in each
// function does not leak across function boundaries. If it did then calling
// the #[async] version of __await in the #[async_move] function should cause
// a 'borrow may still be in use when generator yields' error, while calling
// the #[async_move] version in the #[async] function should fail because `bar`
// does not implement `Future`.
#[async_move]
fn qux2(x: i32) -> Result<i32, i32> {
    #[async]
    fn baz2(x: i32) -> Result<i32, i32> {
        await!(bar(&x))
    }
    await!(baz2(x).pin())
}

#[async_stream(item = u64)]
fn _stream1() -> Result<(), i32> {
    fn integer() -> u64 { 1 }
    let x = &integer();
    stream_yield!(0);
    stream_yield!(*x);
    Ok(())
}

#[async]
pub fn uses_async_for() -> Result<Vec<u64>, i32> {
    let mut v = vec![];
    #[async]
    for i in _stream1() {
        v.push(i);
    }
    Ok(v)
}

#[test]
fn main() {
    assert_eq!(block_on_stable(foo()), Ok(1));
    assert_eq!(block_on_stable(bar(&1)), Ok(1));
    assert_eq!(block_on_stable(baz(17)), Ok(17));
    assert_eq!(block_on_stable(qux(17)), Ok(17));
    assert_eq!(block_on_stable(qux2(17)), Ok(17));
    assert_eq!(block_on_stable(uses_async_for()), Ok(vec![0, 1]));
}
