//! A bunch of ways to use async/await syntax.
//!
//! This is mostly a test f r this repository itself, not necessarily serving
//! much more purpose than that.

use futures;
use futures::executor;
use futures::stable::block_on_stable;

use std::io;

use futures::Never;
use futures::future::poll_fn;

#[async]
fn foo() -> Result<i32, i32> {
    Ok(1)
}

#[async]
extern fn _foo1() -> Result<i32, i32> {
    Ok(1)
}

#[async]
unsafe fn _foo2() -> io::Result<i32> {
    Ok(1)
}

#[async]
unsafe extern fn _foo3() -> io::Result<i32> {
    Ok(1)
}

#[async]
pub fn _foo4() -> io::Result<i32> {
    Ok(1)
}

#[async]
fn _foo5<T: Clone + 'static>(t: T) -> Result<T, i32> {
    Ok(t.clone())
}

#[async]
fn _foo6(ref a: i32) -> Result<i32, i32> {
    Err(*a)
}

#[async]
fn _foo7<T>(t: T) -> Result<T, i32>
    where T: Clone + 'static,
{
    Ok(t.clone())
}

#[async(boxed)]
fn _foo8(a: i32, b: i32) -> Result<i32, i32> {
    return Ok(a + b)
}

#[async(boxed, send)]
fn _foo9() -> Result<(), Never> {
    Ok(())
}

#[async]
fn _bar() -> Result<i32, i32> {
    foo().await
}

#[async]
fn _bar2() -> Result<i32, i32> {
    let a = foo().await?;
    let b = foo().await?;
    Ok(a + b)
}

#[async]
fn _bar4() -> Result<i32, i32> {
    let mut cnt = 0;
    #[async]
    for x in futures::stream::iter_ok::<_, i32>(vec![1, 2, 3, 4]) {
        cnt += x;
    }
    Ok(cnt)
}

#[async_stream(item = u64)]
fn _stream1() -> Result<(), i32> {
    stream_yield!(0);
    stream_yield!(1);
    Ok(())
}

#[async_stream(item = T)]
fn _stream2<T: Clone>(t: T) -> Result<(), i32> {
    stream_yield!(t.clone());
    stream_yield!(t.clone());
    Ok(())
}

#[async_stream(item = i32)]
fn _stream3() -> Result<(), i32> {
    let mut cnt = 0;
    #[async]
    for x in futures::stream::iter_ok::<_, i32>(vec![1, 2, 3, 4]) {
        cnt += x;
        stream_yield!(x);
    }
    Err(cnt)
}

#[async_stream(boxed, item = u64)]
fn _stream4() -> Result<(), i32> {
    stream_yield!(0);
    stream_yield!(1);
    Ok(())
}

#[allow(dead_code)]
mod foo { pub struct Foo(pub i32); }

#[allow(dead_code)]
#[async_stream(boxed, item = foo::Foo)]
pub fn stream5() -> Result<(), i32> {
    stream_yield!(foo::Foo(0));
    stream_yield!(foo::Foo(1));
    Ok(())
}

#[async_stream(boxed, item = i32)]
pub fn _stream6() -> Result<(), i32> {
    #[async]
    for foo::Foo(i) in stream5() {
        stream_yield!(i * i);
    }
    Ok(())
}

#[async_stream(item = ())]
pub fn _stream7() -> Result<(), i32> {
    stream_yield!(());
    Ok(())
}

#[async_stream(item = [u32; 4])]
pub fn _stream8() -> Result<(), i32> {
    stream_yield!([1, 2, 3, 4]);
    Ok(())
}

struct A(i32);

impl A {
    #[async]
    fn a_foo(self) -> Result<i32, i32> {
        Ok(self.0)
    }

    #[async]
    fn _a_foo2(self: Box<Self>) -> Result<i32, i32> {
        Ok(self.0)
    }
}

fn await_item_stream() -> impl Stream<Item = u64, Error = Never> + Send {
    ::futures::stream::iter_ok(vec![0, 1])
}

#[async]
fn test_await_item() -> Result<(), Never> {
    let mut stream = await_item_stream();

    assert_eq!(await_item!(stream), Ok(Some(0)));
    assert_eq!(await_item!(stream), Ok(Some(1)));
    assert_eq!(await_item!(stream), Ok(None));

    Ok(())
}

#[test]
fn main() {
    assert_eq!(block_on_stable(foo()), Ok(1));
    assert_eq!(block_on_stable(foo()), Ok(1));
    assert_eq!(block_on_stable(_bar()), Ok(1));
    assert_eq!(block_on_stable(_bar2()), Ok(2));
    assert_eq!(block_on_stable(_bar4()), Ok(10));
    assert_eq!(block_on_stable(_foo6(8)), Err(8));
    assert_eq!(block_on_stable(A(11).a_foo()), Ok(11));
    assert_eq!(block_on_stable(loop_in_loop()), Ok(true));
    assert_eq!(block_on_stable(test_await_item()), Ok(()));
}

#[async]
fn loop_in_loop() -> Result<bool, i32> {
    let mut cnt = 0;
    let vec = vec![1, 2, 3, 4];
    #[async]
    for x in futures::stream::iter_ok::<_, i32>(vec.clone()) {
        #[async]
        for y in futures::stream::iter_ok::<_, i32>(vec.clone()) {
            cnt += x * y;
        }
    }

    let sum = (1..5).map(|x| (1..5).map(|y| x * y).sum::<i32>()).sum::<i32>();
    Ok(cnt == sum)
}

#[async_stream(item = i32)]
fn poll_stream_after_error_stream() -> Result<(), ()> {
    stream_yield!(5);
    Err(())
}

#[test]
fn poll_stream_after_error() {
    let mut s = poll_stream_after_error_stream().pin();
    assert_eq!(executor::block_on(poll_fn(|ctx| s.poll_next(ctx))), Ok(Some(5)));
    assert_eq!(executor::block_on(poll_fn(|ctx| s.poll_next(ctx))), Err(()));
    assert_eq!(executor::block_on(poll_fn(|ctx| s.poll_next(ctx))), Ok(None));
}
