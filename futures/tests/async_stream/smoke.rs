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

#[async_stream]
fn stream1() -> u64 {
    stream_yield!(0);
    stream_yield!(1);
}

#[async_stream]
fn stream2<T: Clone>(t: T) -> T {
    stream_yield!(t.clone());
    stream_yield!(t.clone());
}

#[async_stream]
fn stream3() -> i32 {
    let mut cnt = 0;
    #[for_await]
    for x in stream::iter(vec![1, 2, 3, 4]) {
        cnt += x;
        stream_yield!(x);
    }
    stream_yield!(cnt);
}

mod foo {
    pub struct _Foo(pub i32);
}

#[async_stream]
fn _stream5() -> foo::_Foo {
    stream_yield!(foo::_Foo(0));
    stream_yield!(foo::_Foo(1));
}

#[async_stream]
fn _stream6() -> i32 {
    #[for_await]
    for foo::_Foo(i) in _stream5() {
        stream_yield!(i * i);
    }
}

#[async_stream]
fn _stream7() -> () {
    stream_yield!(());
}

#[async_stream]
fn _stream8() -> [u32; 4] {
    stream_yield!([1, 2, 3, 4]);
}

struct A(i32);

impl A {
    #[async_stream]
    fn a_foo(self) -> i32 {
        stream_yield!(self.0)
    }

    #[async_stream]
    fn _a_foo2(self: Box<Self>) -> i32 {
        stream_yield!(self.0)
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

fn await_item_stream() -> impl Stream<Item = u64> {
    stream::iter(vec![0, 1])
}

async fn test_await_item() {
    let mut stream = await_item_stream();

    assert_eq!(await_item!(&mut stream), Some(0));
    assert_eq!(await_item!(&mut stream), Some(1));
    assert_eq!(await_item!(&mut stream), None);
    assert_eq!(await_item!(&mut stream), None);
}

#[test]
fn main() {
    // https://github.com/alexcrichton/futures-await/issues/45
    #[async_stream]
    fn _stream10() {
        stream_yield!(());
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
    assert_eq!(block_on(test_await_item()), ());
}
