use futures::executor::block_on;
use futures::stream::{self, *};

#[test]
fn select() {
    fn select_and_compare(a: Vec<u32>, b: Vec<u32>, expected: Vec<u32>) {
        let a = stream::iter(a);
        let b = stream::iter(b);
        let vec = block_on(stream::select(a, b).collect::<Vec<_>>());
        assert_eq!(vec, expected);
    }

    select_and_compare(vec![1, 2, 3], vec![4, 5, 6], vec![1, 4, 2, 5, 3, 6]);
    select_and_compare(vec![1, 2, 3], vec![4, 5], vec![1, 4, 2, 5, 3]);
    select_and_compare(vec![1, 2], vec![4, 5, 6], vec![1, 4, 2, 5, 6]);
}

#[test]
fn scan() {
    futures::executor::block_on(async {
        assert_eq!(
            stream::iter(vec![1u8, 2, 3, 4, 6, 8, 2])
                .scan(1, |state, e| {
                    *state += 1;
                    futures::future::ready(if e < *state { Some(e) } else { None })
                })
                .collect::<Vec<_>>()
                .await,
            vec![1u8, 2, 3, 4]
        );
    });
}

#[test]
fn flat_map_unordered() {
    futures::executor::block_on(async {
        let st = stream::iter(vec![
            stream::iter(0..=4u8),
            stream::iter(6..=10),
            stream::iter(0..=2),
        ]);

        let mut fm_unordered = st
            .flat_map_unordered(1, |s| s.filter(|v| futures::future::ready(v % 2 == 0)))
            .collect::<Vec<_>>()
            .await;

        fm_unordered.sort();

        assert_eq!(fm_unordered, vec![0, 0, 2, 2, 4, 6, 8, 10]);
    });
}
