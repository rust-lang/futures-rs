use futures::executor::block_on;
use futures::*;

struct Ref<'a, T>(&'a T);

#[async_stream(item = i32)]
fn references(x: &i32) {
    yield *x;
}

#[async_stream(item = i32)]
fn new_types(x: Ref<'_, i32>) {
    yield *x.0;
}

struct Foo(i32);

impl Foo {
    #[async_stream(item = &i32)]
    fn foo(&self) {
        yield &self.0
    }
}

#[async_stream(item = &i32)]
fn single_ref(x: &i32) {
    yield x
}

#[async_stream(item = ())]
fn check_for_name_colision<'_async0, T>(_x: &T, _y: &'_async0 i32) {
    yield
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
