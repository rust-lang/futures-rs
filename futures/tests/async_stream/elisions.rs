use futures::executor::block_on;
use futures::*;

struct Ref<'a, T>(&'a T);

#[async_stream]
fn references(x: &i32) -> i32 {
    stream_yield!(*x);
}

#[async_stream]
fn new_types(x: Ref<'_, i32>) -> i32 {
    stream_yield!(*x.0);
}

struct Foo(i32);

impl Foo {
    #[async_stream]
    fn foo(&self) -> &i32 {
        stream_yield!(&self.0)
    }
}

#[async_stream]
fn single_ref(x: &i32) -> &i32 {
    stream_yield!(x)
}

#[async_stream]
fn check_for_name_colision<'_async0, T>(_x: &T, _y: &'_async0 i32) {
    stream_yield!(())
}

#[test]
fn main() {
    block_on(async {
        let x = 0;
        let foo = Foo(x);

        #[for_await]
        for y in references(&x) {
            assert_eq!(y, x);
        }

        #[for_await]
        for y in new_types(Ref(&x)) {
            assert_eq!(y, x);
        }

        #[for_await]
        for y in single_ref(&x) {
            assert_eq!(y, &x);
        }

        #[for_await]
        for y in foo.foo() {
            assert_eq!(y, &x);
        }

        #[for_await]
        for y in check_for_name_colision(&x, &x) {
            assert_eq!(y, ());
        }
    });
}
