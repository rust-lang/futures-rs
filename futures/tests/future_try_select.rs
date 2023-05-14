use futures::future::{ok, try_select};
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
