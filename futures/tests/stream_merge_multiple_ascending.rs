use futures::executor::block_on;
use futures::stream::{self, merge_multiple_ascending, StreamExt};
use futures::Future;
use futures_util::gen_index;
use std::thread;
use std::time::Duration;

#[test]
fn test_merge_multiple_disjoint_streams() {
    let s1 = stream::iter(0..10);
    let s2 = stream::iter(10..20);
    let s3 = stream::iter(20..30);
    let collected = block_on(merge_multiple_ascending([s1, s2, s3]).collect::<Vec<_>>());
    assert_eq!(collected, (0..30).collect::<Vec<_>>());
}

#[test]
fn test_merge_multiple_overlapping_streams() {
    let s1 = stream::iter((0..10).map(|x| x * 3).collect::<Vec<_>>());
    let s2 = stream::iter((0..10).map(|x| x * 3 + 1).collect::<Vec<_>>());
    let s3 = stream::iter((0..10).map(|x| x * 3 + 2).collect::<Vec<_>>());
    let collected = block_on(merge_multiple_ascending([s1, s2, s3]).collect::<Vec<_>>());
    assert_eq!(collected, (0..30).collect::<Vec<_>>());
}

#[test]
fn test_merge_multiple_duplicate_item_streams() {
    let s1 = stream::iter(0..5);
    let s2 = stream::iter(0..5);
    let s3 = stream::iter(0..5);
    let collected = block_on(merge_multiple_ascending([s1, s2, s3]).collect::<Vec<_>>());
    assert_eq!(collected, vec![0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4, 4]);
}

#[test]
fn test_merge_multiple_delayed_streams() {
    type BoxedUnfoldGenerator<T, S> =
        Box<dyn Fn(S) -> std::pin::Pin<Box<dyn Future<Output = Option<(T, S)>>>>>;
    let f1: BoxedUnfoldGenerator<i32, i32> = Box::new(|state| {
        Box::pin(async move {
            if state >= 5 {
                None
            } else {
                thread::sleep(Duration::from_millis(gen_index(500) as u64));
                Some((state * 3, state + 1))
            }
        })
    });
    let f2: BoxedUnfoldGenerator<i32, i32> = Box::new(|state: i32| {
        Box::pin(async move {
            if state >= 5 {
                None
            } else {
                thread::sleep(Duration::from_millis(gen_index(500) as u64));
                Some((state * 3 + 1, state + 1))
            }
        })
    });
    let f3: BoxedUnfoldGenerator<i32, i32> = Box::new(|state: i32| {
        Box::pin(async move {
            if state >= 5 {
                None
            } else {
                thread::sleep(Duration::from_millis(gen_index(500) as u64));
                Some((state * 3 + 2, state + 1))
            }
        })
    });
    let s1 = stream::unfold(0, f1);
    let s2 = stream::unfold(0, f2);
    let s3 = stream::unfold(0, f3);
    let collected = block_on(merge_multiple_ascending([s1, s2, s3]).collect::<Vec<_>>());
    assert_eq!(collected, (0..15).collect::<Vec<_>>());
}
