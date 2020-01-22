use futures::executor::block_on;
use futures::future::*;
use futures::stream::{self, StreamExt};

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

async fn async_scan_fn(state: &mut u8, e: u8) -> Option<u8> {
    *state += 1;
    if e < *state {
        Some(e)
    } else {
        None
    }
}

fn impl_scan_fn<'a>(state: &'a mut u8, e: u8) -> impl Future<Output = Option<u8>> + 'a {
    async move {
        *state += 1;
        if e < *state {
            Some(e)
        } else {
            None
        }
    }
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
fn scan_with_async() {
    futures::executor::block_on(async {
        assert_eq!(
            stream::iter(vec![1u8, 2, 3, 4, 6, 8, 2])
                .scan(1u8, async_scan_fn)
                .collect::<Vec<_>>()
                .await,
            vec![1u8, 2, 3, 4]
        );
    });
}

#[test]
fn scan_with_impl() {
    futures::executor::block_on(async {
        assert_eq!(
            stream::iter(vec![1u8, 2, 3, 4, 6, 8, 2])
                .scan(1u8, impl_scan_fn)
                .collect::<Vec<_>>()
                .await,
            vec![1u8, 2, 3, 4]
        );
    });
}
