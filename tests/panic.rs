extern crate futures;
extern crate support;

use futures::*;

fn assert_panic<F>(mut f: F)
    where F: Future<Item=u32, Error=u32>
{
    support::assert_panic(f.poll(&Tokens::all()).expect("should be ready"))
}

#[test]
fn combinators_catch() {
    assert_panic(finished(1).map(|_| panic!()));
    assert_panic(failed(1).map_err(|_| panic!()));
    assert_panic(finished(1).and_then(|_| -> Done<u32, u32> { panic!() }));
    assert_panic(failed(1).or_else(|_| -> Done<u32, u32> { panic!() }));
    assert_panic(finished::<u32, u32>(1).then(|_| -> Done<u32, u32> { panic!() }));
    assert_panic(lazy(|| -> Done<u32, u32> { panic!() }));
}
