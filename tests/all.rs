extern crate futures;

use futures::*;

fn is_future_v<A, B, C>(_: C)
    where A: Send + 'static,
          B: Send + 'static,
          C: Future<Item=A, Error=B>
{}

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
    assert_eq!(get(f_ok.select(empty())), Ok(1));
    assert_eq!(get(empty().select(f_ok)), Ok(1));
    assert_eq!(get(f_ok.join(f_err)), Err(1));
    assert_eq!(get(f_ok.join(Ok(2))), Ok((1, 2)));
    assert_eq!(get(f_err.join(f_ok)), Err(1));
}

#[test]
fn test_empty() {
    let f_ok: FutureResult<i32, i32> = Ok(1).into_future();
    let f_err: FutureResult<i32, i32> = Err(1).into_future();
    let empty: Empty<i32, i32> = empty();

    assert!(empty.select(empty).poll().is_err());
    assert!(empty.join(empty).poll().is_err());
    assert!(empty.or_else(move |_| empty).poll().is_err());
    assert!(empty.and_then(move |_| empty).poll().is_err());
    assert!(f_err.or_else(move |_| empty).poll().is_err());
    assert!(f_ok.and_then(move |_| empty).poll().is_err());
    assert!(empty.map(|a| a + 1).poll().is_err());
    assert!(empty.map_err(|a| a + 1).poll().is_err());
    // assert!(empty.cancellable().poll().is_err());
}

// #[test]
// fn test_cancel() {
//     let f_ok: FutureResult<i32, i32> = Ok(1).into_future();
//     let f_err: FutureResult<i32, i32> = Err(1).into_future();
//
//     assert_eq!(get(f_ok.cancellable()), Ok(1));
//     assert_eq!(get(f_err.cancellable()), Err(CancelError::Other(1)));
//     let mut f = f_ok.cancellable();
//     f.cancel();
//     assert_eq!(get(f), Err(CancelError::Cancelled));
//     let mut f = f_err.cancellable();
//     f.cancel();
//     assert_eq!(get(f), Err(CancelError::Cancelled));
// }

#[test]
fn test_collect() {
    let f_ok1: FutureResult<i32, i32> = Ok(1).into_future();
    let f_ok2: FutureResult<i32, i32> = Ok(2).into_future();
    let f_ok3: FutureResult<i32, i32> = Ok(3).into_future();
    let f_err1: FutureResult<i32, i32> = Err(1).into_future();

    assert_eq!(get(collect(vec![f_ok1, f_ok2, f_ok3])), Ok(vec![1, 2, 3]));
    assert_eq!(get(collect(vec![f_ok1, f_err1, f_ok3])), Err(1));
}

#[test]
fn test_finished() {
    assert_eq!(get(finished::<_, i32>(1)), Ok(1));
    assert_eq!(get(failed::<i32, _>(1)), Err(1));
}
