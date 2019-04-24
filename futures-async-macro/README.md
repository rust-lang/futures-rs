
**Note that these are experimental APIs.**

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
futures-preview = { version = "0.3.0-alpha.14", features = ["async-stream", "nightly"] }
```

### \#\[for_await\]

Processes streams using a for loop.

This is a reimplement of [futures-await]'s `#[async]` for loops for futures 0.3 and is an experimental implementation of [the idea listed as the next step of async/await](https://github.com/rust-lang/rfcs/blob/master/text/2394-async_await.md#for-await-and-processing-streams).

```rust
use futures::for_await;
use futures::prelude::*;

#[for_await]
for value in stream::iter(1..=5) {
    println!("{}", value);
}
```

`value` has the `Item` type of the stream passed in. Note that async for loops can only be used inside of `async` functions or `#[async_stream]` functions.

### \#\[async_stream\]

This is a reimplement of [futures-await]'s `#[async_stream]` for futures 0.3 and is an experimental implementation of [the idea listed as the next step of async/await](https://github.com/rust-lang/rfcs/blob/master/text/2394-async_await.md#generators-and-streams).

```rust
use futures::prelude::*;
use futures::{async_stream, stream_yield};

// Returns a stream of i32
#[async_stream]
fn foo(stream: impl Stream<Item = String>) -> i32 {
    #[for_await]
    for x in stream {
        stream_yield!(x.parse().unwrap());
    }
}
```

`#[async_stream]` have an item type specified via `-> some::Path` and the values output from the stream must be yielded via the `stream_yield!` macro.

[futures-await]: https://github.com/alexcrichton/futures-await
