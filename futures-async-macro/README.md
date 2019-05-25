
**Note that these are experimental APIs.**

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
futures-preview = { version = "=0.3.0-alpha.16", features = ["async-stream", "nightly"] }
```

### \#\[for_await\]

Processes streams using a for loop.

This is a reimplement of [futures-await]'s `#[async]` for loops for futures 0.3 and is an experimental implementation of [the idea listed as the next step of async/await](https://github.com/rust-lang/rfcs/blob/master/text/2394-async_await.md#for-await-and-processing-streams).

```rust
#![feature(async_await, stmt_expr_attributes, proc_macro_hygiene)]
use futures::for_await;
use futures::prelude::*;

#[for_await]
for value in stream::iter(1..=5) {
    println!("{}", value);
}
```

`value` has the `Item` type of the stream passed in. Note that async for loops can only be used inside of `async` functions, closures, blocks, `#[async_stream]` functions and `async_stream_block!` macros.

### \#\[async_stream\]

Creates streams via generators.

This is a reimplement of [futures-await]'s `#[async_stream]` for futures 0.3 and is an experimental implementation of [the idea listed as the next step of async/await](https://github.com/rust-lang/rfcs/blob/master/text/2394-async_await.md#generators-and-streams).

```rust
#![feature(async_await, generators)]
use futures::prelude::*;
use futures::async_stream;

// Returns a stream of i32
#[async_stream(item = i32)]
fn foo(stream: impl Stream<Item = String>) {
    #[for_await]
    for x in stream {
        yield x.parse().unwrap();
    }
}
```

`#[async_stream]` must have an item type specified via `item = some::Path` and the values output from the stream must be yielded via the `yield` expression.

[futures-await]: https://github.com/alexcrichton/futures-await
