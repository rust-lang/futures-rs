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

## What's next?

This crate is very much in flux at the moment due to everything being super
unstable and nothing's even in the master branch of rust-lang/rust. I hope to
get this more "stable" on nightly at least in terms of breaking less often as
well as fixing bugs that arise.

At the moment it's definitely not recommended to use this in production at all,
if you'd like though please feel more than welcome to try it out! If you run
into any questions or have any issues, you're more than welcome to [open an
issue][issue]!

[issue]: https://github.com/alexcrichton/futures-await/issues/new

## Caveats

As can be expected with many nightly features, there are a number of caveats to
be aware of when working with this project. The main one is that this is early
stages work. Nothing is upstream yet. Things will break! That being said bug
reports are more than welcome, always good to know what needs to be rixed
regardless!

### Compiler errors

Compiler error messages aren't the best. Right now the `proc-macro` system in
the compiler will attribute all compiler errors in a function to the literal
`#[async]` label. For example this code:

```rust
#[async]
fn foo() -> Result<i32, i32> {
    Ok(e)
}
```

This is missing the definition of the variable `e`, but when run through the
compiler the error looks like:

```
error[E0425]: cannot find value `e` in this scope
 --> examples/foo.rs:9:1
  |
9 | #[async]
  | ^^^^^^^^ not found in this scope

error: aborting due to previous error
```

This isn't just a local problem, *all* compiler errors with spans that would
otherwise be in the async function are attributed to the `#[async]` attribute.
This is unfortunately just how procedural macros work today, but lots of work
is happening in the compiler to improve this! As soon as that's available
this crate will try to help take advantage of it.

### Borrowing

Borrowing doesn't really work so well today. The compiler will either reject
many borrows or there may be some unsafety lurking as the generators feature
is being developed. For example if you have a function such as:

```rust
#[async]
fn foo(s: &str) -> io::Result<()> {
    // ..
}
```

This may not compile! The reason for this is that the returned future
typically needs to adhere to the `'static` bound. Async functions currently
execute no code when called, they only make progress when polled. This means
that when you call an async function what happens is it creates a future,
packages up the arguments, and then returns that to you. In this case it'd
have to return the `&str` back up, which doesn't always have the lifetimes
work out.

An example of how to get the above function compiling would be to do:

```rust
#[async]
fn foo(s: String) -> io::Result<()> {
    // ...
}
```

or somehow otherwise use an owned value instead of a borrowed reference.

Note that arguments are not the only point of pain with borrowing. For example
code like this will (or at least shouldn't) compile today:

```rust
for line in string.lines() {
    await!(process(line));
    println!("processed: {}", line);
}
```

The problem here is that `line` is a borrowed value that is alive across a yield
point, namely the call to `await!`. This means that when the future may return
back up the stack was part of the `await!` process it'd have to restore the
`line` variable when it reenters the future. This isn't really all implemented
and may be unsafe today. As a rule of thumb for now you'll need to only have
owned values (no borrowed internals) alive across calls to `await!` or during
async `for` loops.

Lots of thought is being put in to figure out how to alleviate this restriction!
Borrowing is the crux of many ergonomic patterns in Rust, and we'd like this to
work!

As one final point, a consequence of the "no borrowed arguments" today is that
function signatures like:

```rust
#[async]
fn foo(&self) -> io::Result<()> {
    // ...
}
```

unfortunately will not work. You'll either need to take `self` by value or defer
to a different `#[async]` function.

# License

`futures-await` is primarily distributed under the terms of both the MIT
license and the Apache License (Version 2.0), with portions covered by various
BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
