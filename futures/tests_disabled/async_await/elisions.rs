use futures::stable::block_on_stable;

struct Ref<'a, T: 'a>(&'a T);

#[async]
fn references(x: &i32) -> Result<i32, i32> {
    Ok(*x)
}

#[async]
fn new_types(x: Ref<'_, i32>) -> Result<i32, i32> {
    Ok(*x.0)
}

#[async_stream(item = i32)]
fn _streams(x: &i32) -> Result<(), i32> {
    stream_yield!(*x);
    Ok(())
}

struct Foo(i32);

impl Foo {
    #[async]
    fn foo(&self) -> Result<&i32, i32> {
        Ok(&self.0)
    }
}

#[async]
fn single_ref(x: &i32) -> Result<&i32, i32> {
    Ok(x)
}

#[async]
fn check_for_name_colision<'_async0, T>(_x: &T, _y: &'_async0 i32) -> Result<(), ()> {
    Ok(())
}

#[test]
fn main() {
    let x = 0;
    let foo = Foo(x);
    assert_eq!(block_on_stable(references(&x)), Ok(x));
    assert_eq!(block_on_stable(new_types(Ref(&x))), Ok(x));
    assert_eq!(block_on_stable(single_ref(&x)), Ok(&x));
    assert_eq!(block_on_stable(foo.foo()), Ok(&x));
    assert_eq!(block_on_stable(check_for_name_colision(&x, &x)), Ok(()));
}
