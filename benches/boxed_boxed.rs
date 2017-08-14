#![feature(test)]

extern crate test;
extern crate futures;

use test::Bencher;

use futures::Async;
use futures::stream;
use futures::stream::Stream;


fn drain<S : Stream<Item=u32, Error=()>>(mut s: S) {
    loop {
        match s.poll() {
            Ok(Async::Ready(Some(i))) => { test::black_box(i); },
            Ok(Async::Ready(None)) => return,
            _ => unreachable!(),
        }
    }
}

#[bench]
fn plain(b: &mut Bencher) {
    b.iter(|| {
        drain(stream::iter((0..1000).map(Ok)));
    })
}

#[bench]
fn boxed(b: &mut Bencher) {
    b.iter(|| {
        drain(stream::iter((0..1000).map(Ok)).boxed());
    })
}

#[bench]
fn boxed_boxed(b: &mut Bencher) {
    b.iter(|| {
        drain(stream::iter((0..1000).map(Ok)).boxed().boxed());
    })
}

#[bench]
fn boxed_boxed_boxed(b: &mut Bencher) {
    b.iter(|| {
        drain(stream::iter((0..1000).map(Ok)).boxed().boxed().boxed());
    })
}
