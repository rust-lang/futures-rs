use futures::executor::block_on;
use futures::stream::{self, diff_ascending, StreamExt};
use futures_util::gen_index;
use std::thread;
use std::time::Duration;

#[test]
fn test_diff_identical_streams() {
    let s1 = stream::iter(0..5);
    let s2 = stream::iter(0..5);
    let collected = block_on(diff_ascending(s1, s2).collect::<Vec<_>>());
    assert!(collected.is_empty());
}

#[test]
fn test_diff_streams_superset() {
    let s1 = stream::iter(0..5);
    let s2 = stream::iter(0..10);
    let collected = block_on(diff_ascending(s1, s2).collect::<Vec<_>>());
    assert!(collected.is_empty());
}

#[test]
fn test_diff_streams_subset() {
    let s1 = stream::iter(0..10);
    let s2 = stream::iter(0..5);
    let collected = block_on(diff_ascending(s1, s2).collect::<Vec<_>>());
    assert_eq!(collected, vec![5, 6, 7, 8, 9]);
}

#[test]
fn test_diff_delayed_streams() {
    let random_delay = || thread::sleep(Duration::from_millis(gen_index(500) as u64));
    let s1 = stream::unfold(0, |state| async move {
        if state >= 20 {
            None
        } else {
            random_delay();
            Some((state, state + 1))
        }
    });
    let s2 = stream::unfold(0, |state| async move {
        if state >= 10 {
            None
        } else {
            random_delay();
            Some((state, state + 1))
        }
    });
    let collected = block_on(diff_ascending(s1, s2).collect::<Vec<_>>());
    assert_eq!(collected, (10..20).collect::<Vec<_>>());
}
