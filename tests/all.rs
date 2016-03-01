extern crate futures;

use futures::*;

fn is_future_v<A, B, C: Future<Item=A, Error=B>>(_: C) {}

fn get<F: Future>(f: F) -> Result<F::Item, F::Error> {
    f.poll().ok().unwrap()
}

#[test]
fn result_smoke() {
    let f = Ok(1).into_future();

    is_future_v::<i32, u32, _>(f.map(|a| a + 1));
    is_future_v::<i32, u32, _>(f.map_err(|a| a + 1));
    is_future_v::<i32, u32, _>(f.and_then(|a| Ok(a)));
    is_future_v::<i32, u32, _>(f.or_else(|a| Err(a)));
    // is_future_v::<i32, u32, _>(f.on_success(|_| ()));
    // is_future_v::<i32, u32, _>(f.on_error(|_| ()));
    is_future_v::<i32, u32, _>(f.select(Err(3)));
    is_future_v::<(i32, i32), u32, _>(f.join(Err(3)));


    let f_ok: FutureResult<i32, i32> = Ok(1).into_future();
    let f_err: FutureResult<i32, i32> = Err(1).into_future();

    fn ok(a: i32) -> Result<i32, i32> { Ok(a) }
    fn err(a: i32) -> Result<i32, i32> { Err(a) }

    assert_eq!(get(f_ok.map(|a| a + 2)), ok(3));
    assert_eq!(get(f_err.map(|a| a + 2)), err(1));
    assert_eq!(get(f_ok.map_err(|a| a + 2)), ok(1));
    assert_eq!(get(f_err.map_err(|a| a + 2)), err(3));
    assert_eq!(get(f_ok.and_then(|a| Ok(a + 2))), ok(3));
    assert_eq!(get(f_err.and_then(|a| Ok(a + 2))), err(1));
    assert_eq!(get(f_ok.and_then(|a| Err(a + 3))), err(4));
    assert_eq!(get(f_err.and_then(|a| Err(a + 4))), err(1));
    assert_eq!(get(f_ok.or_else(|a| Ok(a + 2))), ok(1));
    assert_eq!(get(f_err.or_else(|a| Ok(a + 2))), ok(3));
    assert_eq!(get(f_ok.or_else(|a| Err(a + 3))), ok(1));
    assert_eq!(get(f_err.or_else(|a| Err(a + 4))), err(5));
    assert_eq!(get(f_ok.select(f_err)), ok(1));
    assert_eq!(get(f_ok.select(Ok(2))), ok(1));
    assert_eq!(get(f_err.select(f_ok)), err(1));
    assert_eq!(get(f_ok.select(Empty::new())), Ok(1));
    assert_eq!(get(Empty::new().select(f_ok)), Ok(1));
    assert_eq!(get(f_ok.join(f_err)), Err(1));
    assert_eq!(get(f_ok.join(Ok(2))), Ok((1, 2)));
    assert_eq!(get(f_err.join(f_ok)), Err(1));

    let empty: Empty<i32, i32> = Empty::new();
    assert!(empty.select(empty).poll().is_err());
    assert!(empty.join(empty).poll().is_err());
    assert!(empty.or_else(|_| empty).poll().is_err());
    assert!(empty.and_then(|_| empty).poll().is_err());
    assert!(f_err.or_else(|_| empty).poll().is_err());
    assert!(f_ok.and_then(|_| empty).poll().is_err());
    assert!(empty.map(|a| a + 1).poll().is_err());
    assert!(empty.map_err(|a| a + 1).poll().is_err());
}
