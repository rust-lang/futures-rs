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
compiler from this branch https://github.com/Zoxc/rust/tree/gen.

```
git clone https://github.com/Zoxc/rust --branch gen
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

For a whole mess of examples in a whole mess of code, you can also check out the
[`async-await` branch of `sccache`][branch] which is an in-progress transition
to using async/await syntax in many locations. You'll typically find that the
code is much more readable afterwards, especially [these changes]

[branch]: https://github.com/alexcrichton/sccache/tree/async-await
[these changes]: https://github.com/alexcrichton/sccache/commit/927fe00d466ce8a61c37e48c236ac5fe82cb6280#diff-67d38c24e74f3822389d7fe6916b9e69L98

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

### Futures in traits

Let's say you've got a trait like so:

```rust
trait MyStuff {
    fn do_async_task(??self) -> Box<Future<...>>;
}
```

We'll gloss over the `self` details here for a bit, but in essence we've got a
function in a trait tha twants to return a future. Unfortunately it's actually
quite difficult to use this! Right now there's a few of caveats:

* Ideally you want to tag this `#[async]` but this is (a) not implemented in the
  procedural macro right now (it doesn't rewrite trait function declarations)
  but also (b) it doesn't work because a trait function returning `impl Future`
  is not implemented in the compiler today. I'm told that this will eventually
  work, though!
* Ok so then the next best thing is `#[async(boxed)]` to return a boxed trait
  object instead of `impl Future` for the meantime. This still isn't actually
  implemented in the `futures-await` implementation of `#[async]` (it doesn't
  rewrite trait functions) but it's plausible!
* But now this brings us to the handling of `self`. Because of the limitations
  of `#[async]` today we only have two options, `self` and `self: Box<Self>`.
  The former is unfortunately not object safe (now we can't use virtual dispatch
  with this trait) and the latter is typically wasteful (every invocation now
  requires a fresh allocation). Ideally `self: Rc<Self>` is exactly what we want
  here! But unfortunately this isn't implemented in the compiler :frowning:

So basically in summary you've got one of two options to return futures in
traits today:

```rust
trait MyStuff {
    // Trait is not object safe because of `self` so can't have virtual
    // dispatch, and the allocation of `Box<..>` as a return value is required
    // until the compiler implements returning `impl Future` from traits.
    //
    // Note that the upside of this approach, though, is that `self` could be
    // something like `Rc` or have a bunch fo `Rc` inside of `self`, so this
    // could be cheap to call.
    fn do_async_task(self) -> Box<Future<...>>;
}
```

or the alternative:

```rust
trait MyStuff {
    // Like above we returned a trait object but here the trait is indeed object
    // safe, allowing virtual dispatch. The downside is that we must have a
    // `Box` on hand every time we call this function, which may be costly in
    // some situations.
    fn do_async_task(self: Box<Self>) -> Box<Future<...>>;
}
```

The *ideal end goal* for futures-in-traits is this:

```rust
trait MyStuff {
    #[async]
    fn do_async_task(self: Rc<Self>) -> Result<i32, u32>;
}
```

but this needs three pieces to be implemented:

* The compiler must accept trait functions returning `impl Trait`
* The compiler needs support for `self: Rc<Self>`, basically object-safe custom
  smart pointers in traits.
* And finally, the compiler needs to support `proc_macro_attribute` expansion on
  trait functions like this, allowing the `#[async]` implementation to rewrite
  this signature.

# License

`futures-await` is primarily distributed under the terms of both the MIT
license and the Apache License (Version 2.0), with portions covered by various
BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
