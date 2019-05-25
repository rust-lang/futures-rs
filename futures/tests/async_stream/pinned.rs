use futures::executor::block_on;
use futures::*;

#[async_stream(item = u64)]
fn stream1() {
    fn integer() -> u64 {
        1
    }
    let x = &integer();
    yield 0;
    yield *x;
}

fn stream1_block() -> impl Stream<Item = u64> {
    async_stream_block! {
        #[for_await]
        for item in stream1() {
            yield item
        }
    }
}

async fn uses_async_for() -> Vec<u64> {
    let mut v = vec![];
    #[for_await]
    for i in stream1() {
        v.push(i);
    }
    v
}

async fn uses_async_for_block() -> Vec<u64> {
    let mut v = vec![];
    #[for_await]
    for i in stream1_block() {
        v.push(i);
    }
    v
}

#[test]
fn main() {
    assert_eq!(block_on(uses_async_for()), vec![0, 1]);
    assert_eq!(block_on(uses_async_for_block()), vec![0, 1]);
}
