use futures::future::{ok, try_select, try_select_biased};
use futures_executor::block_on;

#[test]
fn is_fair() {
    let mut results = Vec::with_capacity(100);
    for _ in 0..100 {
        let (i, _) = block_on(try_select(ok::<_, ()>(0), ok::<_, ()>(1))).unwrap().factor_first();
        results.push(i);
    }
    const THRESHOLD: usize = 30;
    assert_eq!(results.iter().filter(|i| **i == 0).take(THRESHOLD).count(), THRESHOLD);
    assert_eq!(results.iter().filter(|i| **i == 1).take(THRESHOLD).count(), THRESHOLD)
}

#[test]
fn is_biased() {
    let mut results = Vec::with_capacity(100);
    for _ in 0..100 {
        let (i, _) =
            block_on(try_select_biased(ok::<_, ()>(0), ok::<_, ()>(1))).unwrap().factor_first();
        results.push(i);
    }
    assert!(results.iter().all(|i| *i == 0));
}
