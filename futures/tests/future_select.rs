use std::future::ready;

use futures::future::select;
use futures_executor::block_on;

#[test]
fn is_fair() {
    let mut results = Vec::with_capacity(100);
    for _ in 0..100 {
        let (i, _) = block_on(select(ready(0), ready(1))).factor_first();
        results.push(i);
    }
    const THRESHOLD: usize = 30;
    assert_eq!(results.iter().filter(|i| **i == 0).take(THRESHOLD).count(), THRESHOLD);
    assert_eq!(results.iter().filter(|i| **i == 1).take(THRESHOLD).count(), THRESHOLD)
}
