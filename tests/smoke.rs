//! A bunch of ways to use async/await syntax.
//!
//! This is mostly a test for this repository itself, not necessarily serving
//! much more purpose than that.

#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate futures_await as futures;
extern crate futures_cpupool;

use futures::executor::current_thread;

use std::io;

use futures::future::poll_fn;
use futures::prelude::*;
use futures_cpupool::CpuPool;

#[async_move]
fn foo() -> Result<i32, i32> {
    Ok(1)
}

#[async_move]
extern fn _foo1() -> Result<i32, i32> {
    Ok(1)
}

#[async_move]
unsafe fn _foo2() -> io::Result<i32> {
    Ok(1)
}

#[async_move]
unsafe extern fn _foo3() -> io::Result<i32> {
    Ok(1)
}

#[async_move]
pub fn _foo4() -> io::Result<i32> {
    Ok(1)
}

#[async_move]
fn _foo5<T: Clone + 'static>(t: T) -> Result<T, i32> {
    Ok(t.clone())
}

#[async_move]
fn _foo6(ref a: i32) -> Result<i32, i32> {
    Err(*a)
}

#[async_move]
fn _foo7<T>(t: T) -> Result<T, i32>
    where T: Clone + 'static,
{
    Ok(t.clone())
}

#[async_move(boxed)]
fn _foo8(a: i32, b: i32) -> Result<i32, i32> {
    return Ok(a + b)
}

#[async_move(boxed_send)]
fn _foo9() -> Result<(), ()> {
    Ok(())
}

#[async_move]
fn _bar() -> Result<i32, i32> {
    await!(foo())
}

#[async_move]
fn _bar2() -> Result<i32, i32> {
    let a = await!(foo())?;
    let b = await!(foo())?;
    Ok(a + b)
}

#[async_move]
fn _bar3() -> Result<i32, i32> {
    let (a, b) = await!(foo().join(foo()))?;
    Ok(a + b)
}

#[async_move]
fn _bar4() -> Result<i32, i32> {
    let mut cnt = 0;
    #[async]
    for x in futures::stream::iter_ok::<_, i32>(vec![1, 2, 3, 4]) {
        cnt += x;
    }
    Ok(cnt)
}

#[async_stream_move(item = u64)]
fn _stream1() -> Result<(), i32> {
    stream_yield!(0);
    stream_yield!(1);
    Ok(())
}

#[async_stream_move(item = T)]
fn _stream2<T: Clone + 'static>(t: T) -> Result<(), i32> {
    stream_yield!(t.clone());
    stream_yield!(t.clone());
    Ok(())
}

#[async_stream_move(item = i32)]
fn _stream3() -> Result<(), i32> {
    let mut cnt = 0;
    #[async]
    for x in futures::stream::iter_ok::<_, i32>(vec![1, 2, 3, 4]) {
        cnt += x;
        stream_yield!(x);
    }
    Err(cnt)
}

#[async_stream_move(boxed, item = u64)]
fn _stream4() -> Result<(), i32> {
    stream_yield!(0);
    stream_yield!(1);
    Ok(())
}

mod foo { pub struct Foo(pub i32); }

#[async_stream_move(boxed, item = foo::Foo)]
pub fn stream5() -> Result<(), i32> {
    stream_yield!(foo::Foo(0));
    stream_yield!(foo::Foo(1));
    Ok(())
}

#[async_stream_move(boxed, item = i32)]
pub fn _stream6() -> Result<(), i32> {
    #[async]
    for foo::Foo(i) in stream5() {
        stream_yield!(i * i);
    }
    Ok(())
}

#[async_stream_move(item = ())]
pub fn _stream7() -> Result<(), i32> {
    stream_yield!(());
    Ok(())
}

#[async_stream_move(item = [u32; 4])]
pub fn _stream8() -> Result<(), i32> {
    stream_yield!([1, 2, 3, 4]);
    Ok(())
}

// struct A(i32);
//
// impl A {
//     #[async]
//     fn a_foo(self) -> Result<i32, i32> {
//         Ok(self.0)
//     }
//
//     #[async]
//     fn _a_foo2(self: Box<Self>) -> Result<i32, i32> {
//         Ok(self.0)
//     }
// }

// trait B {
//     #[async]
//     fn b(self) -> Result<i32, i32>;
// }
//
// impl B for A {
//     #[async]
//     fn b(self) -> Result<i32, i32> {
//         Ok(self.0)
//     }
// }

#[async_stream_move(item = u64)]
fn await_item_stream() -> Result<(), i32> {
    stream_yield!(0);
    stream_yield!(1);
    Ok(())
}

#[async_move]
fn test_await_item() -> Result<(), ()> {
    let mut stream = await_item_stream();

    assert_eq!(await_item!(stream), Ok(Some(0)));
    assert_eq!(await_item!(stream), Ok(Some(1)));
    assert_eq!(await_item!(stream), Ok(None));

    Ok(())
}

#[test]
fn main() {
    current_thread::run(|ctx| {
        assert_eq!(ctx.block_on(foo()), Ok(1));
        assert_eq!(ctx.block_on(foo()), Ok(1));
        assert_eq!(ctx.block_on(_bar()), Ok(1));
        assert_eq!(ctx.block_on(_bar2()), Ok(2));
        assert_eq!(ctx.block_on(_bar3()), Ok(2));
        assert_eq!(ctx.block_on(_bar4()), Ok(10));
        assert_eq!(ctx.block_on(_foo6(8)), Err(8));
        // assert_eq!(A(11).a_foo(), 11);
        assert_eq!(ctx.block_on(loop_in_loop()), Ok(true));
        assert_eq!(ctx.block_on(test_await_item()), Ok(()));
    })
}

#[async_move]
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

#[async_stream_move(item = i32)]
fn poll_stream_after_error_stream() -> Result<(), ()> {
    stream_yield!(5);
    Err(())
}

#[test]
fn poll_stream_after_error() {
    current_thread::run(|ctx| {
        let mut s = poll_stream_after_error_stream();
        assert_eq!(ctx.block_on(poll_fn(|ctx| s.poll(ctx))), Ok(Some(5)));
        assert_eq!(ctx.block_on(poll_fn(|ctx| s.poll(ctx))), Err(()));
        assert_eq!(ctx.block_on(poll_fn(|ctx| s.poll(ctx))), Ok(None));
    })
}

#[test]
fn run_boxed_future_in_cpu_pool() {
    current_thread::run(|ctx| {
        let pool = CpuPool::new_num_cpus();
        ctx.block_on(pool.spawn(_foo9())).unwrap();
    })
}
