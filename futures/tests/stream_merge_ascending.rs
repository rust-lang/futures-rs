use futures::executor::block_on;
use futures::stream::{self, merge_ascending, StreamExt};
use futures_util::gen_index;
use std::thread;
use std::time::Duration;

#[test]
fn test_merge_disjoint_streams() {
    let s1 = stream::iter(0..10);
    let s2 = stream::iter(10..20);
    let collected = block_on(merge_ascending(s1, s2).collect::<Vec<_>>());
    assert_eq!(collected, (0..20).collect::<Vec<_>>());
}

#[test]
fn test_merge_overlapping_streams() {
    let s1 = stream::iter((0..10).map(|x| x * 2));
    let s2 = stream::iter((0..10).map(|x| x * 2 + 1));
    let collected = block_on(merge_ascending(s1, s2).collect::<Vec<_>>());
    assert_eq!(collected, (0..20).collect::<Vec<_>>());
}

#[test]
fn test_merge_duplicate_item_streams() {
    let s1 = stream::iter(0..5);
    let s2 = stream::iter(0..5);
    let collected = block_on(merge_ascending(s1, s2).collect::<Vec<_>>());
    assert_eq!(collected, vec![0, 0, 1, 1, 2, 2, 3, 3, 4, 4]);
}

#[test]
fn test_merge_delayed_streams() {
    let random_delay = || thread::sleep(Duration::from_millis(gen_index(500) as u64));
    let s1 = stream::unfold(0, |state| async move {
        if state >= 5 {
            None
        } else {
            random_delay();
            Some((state * 2, state + 1))
        }
    });
    let s2 = stream::unfold(0, |state| async move {
        if state >= 5 {
            None
        } else {
            random_delay();
            Some((state * 2 + 1, state + 1))
        }
    });
    let collected = block_on(merge_ascending(s1, s2).collect::<Vec<_>>());
    assert_eq!(collected, (0..10).collect::<Vec<_>>());
}
