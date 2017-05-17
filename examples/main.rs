#![feature(proc_macro, conservative_impl_trait, generators, generator_trait)]

extern crate futures_await;
extern crate futures;

use std::io;

use futures::Future;
use futures_await::{async, await};

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

#[async]
fn _bar() -> Result<i32, i32> {
    await!(foo())
}

#[async]
fn _bar2() -> Result<i32, i32> {
    let a = await!(foo())?;
    let b = await!(foo())?;
    Ok(a + b)
}

#[async]
fn _bar3() -> Result<i32, i32> {
    let (a, b) = await!(foo().join(foo()))?;
    Ok(a + b)
}

#[async]
fn _bar4() -> Result<i32, i32> {
    let mut cnt = 0;
    #[async]
    for x in futures::stream::iter(vec![Ok::<i32, i32>(1), Ok(2), Ok(3), Ok(4)]) {
        cnt += x;
    }
    Ok(cnt)
}

struct A(i32);

impl A {
    #[async]
    fn a_foo(self) -> Result<i32, i32> {
        Ok(self.0)
    }
}

fn main() {
    assert_eq!(foo().wait(), Ok(1));
    assert_eq!(_bar().wait(), Ok(1));
    assert_eq!(_bar2().wait(), Ok(2));
    assert_eq!(_bar3().wait(), Ok(2));
    assert_eq!(_bar4().wait(), Ok(10));
    assert_eq!(_foo6(8).wait(), Err(8));
    assert_eq!(A(11).a_foo().wait(), Ok(11));
}
