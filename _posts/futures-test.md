## `futures-test`

With `futures 0.3.0-alpha.4` we're releasing a new independent crate
`futures-test` (under the package [`futures-test-preview` on
crates.io](https://crates.io/crates/futures-test-preview)), this will not be
available as part of the `futures` facade, instead it's intended to be used as a
dev dependency when implementing custom futures.

This initial release includes one major capability (along with a couple of
smaller utilities). There are some ideas for other utilities that will be
useful, if you have existing utilities or ideas of what would be useful for you
[please let us know][#1169].

### Testing without `futures-test`

Before getting into the details of `futures-test`, it's useful to go over a
basic technique for testing that may not be known to everyone. A lot of the time
you don't care about all the details of how some async operation executes, just
that it does what it's meant to do. In these cases you can normally setup a test
that doesn't need all the utilities here, you simply use an async block run via
[`futures::executor::block_on`][] and have all your assertions in there.
Here's a quick example I prepared earlier:

```rust
async fn some_operation<F: FnMut(usize)>(callback: F) -> io::Result<usize> {
    await!(async { /* Some IO stuff */ });
    callback(5);
    await!(async { /* Some more IO */ });
    callback(6);
    await!(async { /* ... */ });
    return 7;
}

#[test]
fn test_some_operation() {
    futures::executor::block_on(async {
        let mut values = vec![];
        let result = await!(some_operation(|value| values.push(value)));
        assert_eq!(result, Ok(7);
        assert_eq!(values, vec![5, 6]);
    })
}
```

### Test `Context`s

While you can get a long way by just defining and running async tests, sometimes
you need to be able to introspect on exactly what is happening, e.g. when
testing [`FutureExt::fuse`][] we want to check that calling `poll` after it
completes correctly returns `Poll::Pending`. However, to call `poll` we need to
somehow acquire a `task::Context`. This is where the main capability of
`futures-test` comes in, it can provide you with all sorts of useful contexts
depending on what you are testing. You have two choices to make when you want a
context:

 1. What should happen when a future calls `cx.waker().wake()`?
 2. What should happen when a future calls `cx.spawner().spawn()`?

For both of these there are two very basic implementations provided: panicking
and noop. Sometimes this is all you need, so for example when testing
`FutureExt::fuse` we use [`futures_test::task::panic_context`][] which gives a
context that will panic on either operation:

```rust
#![feature(pin, arbitrary_self_types, futures_api)]

use futures::future::{self, FutureExt};
use futures_test::task::panic_context;

#[test]
fn fuse() {
    let mut future = future::ready::<i32>(2).fuse();
    let cx = &mut panic_context();
    assert!(future.poll_unpin(cx).is_ready());
    assert!(future.poll_unpin(cx).is_pending());
}
```

#### [`WakeCounter`][]

One slightly more specialized `Wake` implementation provided is to count how
many times a task has been woken. This has been used to check that
[`AbortHandle`][] correctly awakens a waiting task when it is aborted. You can
see from this example that setting up a context with this implementation is
slightly more work, you start with a basic `panic_context`, then swap out the
waker with one provided by the `WakeCounter`:

```rust

#![feature(pin, arbitrary_self_types, futures_api)]

use futures::channel::oneshot;
use futures::future::{abortable, Aborted, FutureExt};
use futures::task::Poll;
use futures_test::task::{panic_context, WakeCounter};

#[test]
fn abortable_awakens() {
    let (_tx, a_rx) = oneshot::channel::<()>();
    let (mut abortable_rx, abort_handle) = abortable(a_rx);

    let wake_counter = WakeCounter::new();
    let mut cx = panic_context();
    let cx = &mut cx.with_waker(wake_counter.local_waker());

    assert_eq!(0, wake_counter.count());
    assert_eq!(Poll::Pending, abortable_rx.poll_unpin(cx));
    assert_eq!(0, wake_counter.count());
    abort_handle.abort();
    assert_eq!(1, wake_counter.count());
    assert_eq!(Poll::Ready(Err(Aborted)), abortable_rx.poll_unpin(cx));
}
```

There are some other implementations provided, take a look in the
[`futures_test::task`][] module docs for more info.

### [`FutureTestExt`][]

Along with the context utilities there are some more test specific combinators
provided, these come on an additional `FutureTestExt` trait. For now there are
three provided: 

[`assert_unmoved`][]
: When you're writing custom futures that wrap over futures you want to be sure
  you don't accidentally move them. This combinator will assert this for you
  automatically as it is used to easily detect accidental moves.

[`pending_once`][]
: A lot of the time when testing you will be working with instantly ready
  futures like `future::ready(5)` or `async { 6 }`, other times you need to
  have a future that actually acts like a future. This combinator will return
  `Poll::Pending` once and instantly wake its task up to emulate a real async
  operation.

[`run_in_background`][]
: Sometimes you just don't want to be bothered running some future to
  completion. This will spin up a dedicated executor on a background thread to
  run it to completion for you.

### [`assert_stream_*!`][]

The final minor utility are a trilogy of macros for working with streams, these
macros all together allow you to poll a stream one poll at a time and ensure it
behaves exactly as you imagined:

```rust
#![feature(async_await, futures_api, pin)]
use futures::stream;
use futures_test::future::FutureTestExt;
use futures_test::{
    assert_stream_pending, assert_stream_next, assert_stream_done,
};
use pin_utils::pin_mut;

let mut stream = stream::once((async { 5 }).pending_once());
pin_mut!(stream);

assert_stream_pending!(stream);
assert_stream_next!(stream, 5);
assert_stream_done!(stream);
```

[`futures::executor::block_on`]: https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.4/futures/executor/fn.block_on.html
[`FutureExt::fuse`]: https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.4/futures/future/trait.FutureExt.html#method.fuse
[`futures_test::task::panic_context`]: https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.4/futures_test/task/fn.panic_context.html
[`futures_test::task`]: https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.4/futures_test/task/index.html
[#1169]: https://github.com/rust-lang-nursery/futures-rs/issues/1169
[`WakeCounter`]: https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.4/futures_test/task/struct.WakeCounter.html
[`FutureTestExt`]: https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.4/futures_test/future/trait.FutureTestExt.html
[`AbortHandle`]: https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.4/futures/future/struct.AbortHandle.html
[`assert_unmoved`]: https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.4/futures_test/future/trait.FutureTestExt.html#method.assert_unmoved
[`pending_once`]: https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.4/futures_test/future/trait.FutureTestExt.html#method.pending_once
[`run_in_background`]: https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.4/futures_test/future/trait.FutureTestExt.html#method.run_in_background
[`assert_stream_*!`]: https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.4/futures_test/index.html#macros
