# futures-await

Async/await syntax for Rust and the [`futures`] crate

[`futures`]: https://crates.io/crates/futures

## What is this?

The primary way of working with futures today in Rust is through the various
combinators on the [`Future`] trait. This is not quite "callback hell" but can
sometimes feel like it as the rightward drift of code increases for each new
closure you tack on. The purpose of async/await is to provide a much more
"synchronous" feeling to code while retaining all the power of asynchronous
code!

Here's a small taste of what this crate does:

```rust
#[async]
fn fetch_rust_lang(client: hyper::Client) -> io::Result<String> {
    let response = await!(client.get("https://www.rust-lang.org"))?;
    if !response.status().is_success() {
        return Err(io::Error::new(io::ErrorKind::Other, "request failed"))
    }
    let body = await!(response.body().concat())?;
    let string = String::from_utf8(body)?;
    Ok(string)
}
```

The notable points here are:

* Functions are tagged with `#[async]`, which means that they are *asychronous*
  and return a future when called instead of returning the result listed.
* The `await!` macro allows blocking on a future to completion. This does not
  actually block the thread though, it just "blocks" the future returned from
  `fetch_rust_lang` from continuing. You can think of the `await!` macro as a
  function from a future to its `Result`, consuming the future along the way.
* Error handling is much more natural here in `#[async]` functions. We get to
  use the `?` operator as well as simple `return Err` statements. With the
  combinators in the [`futures`] crate this is generally much more cumbersome.
* The future returned from `fetch_rust_lang` is actually a state machine
  generated at compile time and does not implicit memory allocation. This is
  built on top of generators, described below.

You can also have async methods:

```rust
impl Foo {
    #[async]
    fn do_work(self) -> io::Result<u32> {
        // ...
    }
}
```

You can specify that you actually would prefer a trait object is returned
instead, e.g. `Box<Future<Item = i32, Error = io::Error>>`

```rust
#[async(boxed)]
fn foo() -> io::Result<i32> {
    // ...
}
```

And finally you can also have "async `for` loops" which operate over the
[`Stream`] trait:

```rust
#[async]
for message in stream {
    // ...
}
```

An async `for` loop will propagate errors out of the function so `message` has
the `Item` type of the `stream` passed in. Note that async `for` loops can only
be used inside of an `#[async]` function.

[`Future`]: https://docs.rs/futures/0.1.13/futures/future/trait.Future.html
[`Stream`]: https://docs.rs/futures/0.1.13/futures/stream/trait.Stream.html

## How do I use this?

This implementation is currently fundamentally built on generators/coroutines.
This feature has not landed in the rust-lang/rust master branch of the Rust
compiler. To work with this repository currently you'll need to build your own
compiler from this branch https://github.com/Zoxc/rust/tree/gen3.

```
git clone https://github.com/Zoxc/rust --branch gen3
cd rust
./x.py build
```

Once that's done you can use the compiler in `build/$target/stage2/bin/rustc`.

To actually use this crate you use a skeleton like this:

```toml
# In your Cargo.toml ...
[dependencies]
futures-await = { git = 'https://github.com/alexcrichton/futures-await' }
```

and then...

```rust
#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate futures_await as futures;

use futures::prelude::*;

#[async]
fn foo() -> Result<i32, i32> {
    Ok(1 + await!(bar())?)
}

#[async]
fn bar() -> Result<i32, i32> {
    Ok(2)
}

fn main() {
    assert_eq!(foo().wait(), Ok(3));
}
```

This crate is intended to "masquerade" as the [`futures`] crate, reexporting its
entire hierarchy and just augmenting it with the necessary runtime support, the
`async` attribute, and the `await!` macro. These imports are all contained in
`futures::prelude::*` when you import it.

# License

`futures-await` is primarily distributed under the terms of both the MIT
license and the Apache License (Version 2.0), with portions covered by various
BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
