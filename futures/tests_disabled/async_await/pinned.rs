use futures::stable::block_on_stable;
use futures::executor::{block_on, ThreadPool};

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
    bar(&x).await
}

#[async(boxed)]
fn boxed(x: i32) -> Result<i32, i32> {
    Ok(x)
}

#[async(boxed)]
fn boxed_borrow(x: &i32) -> Result<i32, i32> {
    Ok(*x)
}

#[async(boxed, send)]
fn boxed_send(x: i32) -> Result<i32, i32> {
    Ok(x)
}

#[async(boxed, send)]
fn boxed_send_borrow(x: &i32) -> Result<i32, i32> {
    Ok(*x)
}

#[async(boxed, send)]
fn spawnable() -> Result<(), Never> {
    Ok(())
}

fn baz_block(x: i32) -> impl StableFuture<Item = i32, Error = i32> {
    async_block! {
        bar(&x).await
    }
}

#[async_stream(item = u64)]
fn stream1() -> Result<(), i32> {
    fn integer() -> u64 { 1 }
    let x = &integer();
    stream_yield!(0);
    stream_yield!(*x);
    Ok(())
}

fn stream1_block() -> impl StableStream<Item = u64, Error = i32> {
    async_stream_block! {
        #[async]
        for item in stream1() {
            stream_yield!(item)
        }
        Ok(())
    }
}

#[async_stream(boxed, item = u64)]
fn _stream_boxed() -> Result<(), i32> {
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
    for i in stream1() {
        v.push(i);
    }
    Ok(v)
}

#[async]
pub fn uses_async_for_block() -> Result<Vec<u64>, i32> {
    let mut v = vec![];
    #[async]
    for i in stream1_block() {
        v.push(i);
    }
    Ok(v)
}

#[test]
fn main() {
    assert_eq!(block_on_stable(foo()), Ok(1));
    assert_eq!(block_on_stable(bar(&1)), Ok(1));
    assert_eq!(block_on_stable(baz(17)), Ok(17));
    assert_eq!(block_on(boxed(17)), Ok(17));
    assert_eq!(block_on(boxed_send(17)), Ok(17));
    assert_eq!(block_on(boxed_borrow(&17)), Ok(17));
    assert_eq!(block_on(boxed_send_borrow(&17)), Ok(17));
    assert_eq!(block_on_stable(baz_block(18)), Ok(18));
    assert_eq!(block_on_stable(uses_async_for()), Ok(vec![0, 1]));
    assert_eq!(block_on_stable(uses_async_for_block()), Ok(vec![0, 1]));
}

#[test]
fn run_pinned_future_in_thread_pool() {
    let mut pool = ThreadPool::new().unwrap();
    pool.spawn_pinned(spawnable()).unwrap();
}
