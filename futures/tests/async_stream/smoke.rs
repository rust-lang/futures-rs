//! A bunch of ways to use async/await syntax.
//!
//! This is mostly a test f r this repository itself, not necessarily serving
//! much more purpose than that.

use futures::executor::block_on;
use futures::*;

async fn future1() -> i32 {
    let mut cnt = 0;
    #[for_await]
    for x in stream::iter(vec![1, 2, 3, 4]) {
        cnt += x;
    }
    cnt
}

#[async_stream(item = u64)]
fn stream1() {
    yield 0;
    yield 1;
}

#[async_stream(item = T)]
fn stream2<T: Clone>(t: T) {
    yield t.clone();
    yield t.clone();
}

#[async_stream(item = i32)]
fn stream3() {
    let mut cnt = 0;
    #[for_await]
    for x in stream::iter(vec![1, 2, 3, 4]) {
        cnt += x;
        yield x;
    }
    yield cnt;
}

mod foo {
    pub struct _Foo(pub i32);
}

#[async_stream(item = foo::_Foo)]
fn _stream5() {
    yield foo::_Foo(0);
    yield foo::_Foo(1);
}

#[async_stream(item = i32)]
fn _stream6() {
    #[for_await]
    for foo::_Foo(i) in _stream5() {
        yield i * i;
    }
}

#[async_stream(item = ())]
fn _stream7() {
    yield ();
}

#[async_stream(item = [u32; 4])]
fn _stream8() {
    yield [1, 2, 3, 4];
}

struct A(i32);

impl A {
    #[async_stream(item = i32)]
    fn a_foo(self) {
        yield self.0
    }

    #[async_stream(item = i32)]
    fn _a_foo2(self: Box<Self>) {
        yield self.0
    }
}

async fn loop_in_loop() -> bool {
    let mut cnt = 0;
    let vec = vec![1, 2, 3, 4];
    #[for_await]
    for x in stream::iter(vec.clone()) {
        #[for_await]
        for y in stream::iter(vec.clone()) {
            cnt += x * y;
        }
    }

    let sum = (1..5).map(|x| (1..5).map(|y| x * y).sum::<i32>()).sum::<i32>();
    cnt == sum
}

#[test]
fn main() {
    // https://github.com/alexcrichton/futures-await/issues/45
    #[async_stream(item = ())]
    fn _stream10() {
        yield;
    }

    block_on(async {
        let mut v = 0..=1;
        #[for_await]
        for x in stream1() {
            assert_eq!(x, v.next().unwrap());
        }

        let mut v = [(), ()].iter();
        #[for_await]
        for x in stream2(()) {
            assert_eq!(x, *v.next().unwrap());
        }

        let mut v = [1, 2, 3, 4, 10].iter();
        #[for_await]
        for x in stream3() {
            assert_eq!(x, *v.next().unwrap());
        }

        #[for_await]
        for x in A(11).a_foo() {
            assert_eq!(x, 11);
        }
    });

    assert_eq!(block_on(future1()), 10);
    assert_eq!(block_on(loop_in_loop()), true);
}
