use std::future::ready;

use futures::future::{select, select_biased};
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

#[test]
fn is_biased() {
    let mut results = Vec::with_capacity(100);
    for _ in 0..100 {
        let (i, _) = block_on(select_biased(ready(0), ready(1))).factor_first();
        results.push(i);
    }
    assert!(results.iter().all(|i| *i == 0));
}
