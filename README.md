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
  generated at compile time and does no implicit memory allocation. This is
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
This feature has [just landed][genpr] and will be in the nightly channel of Rust
as of 2017-08-29. You can acquire the nightly channel via;

[genpr]: https://github.com/rust-lang/rust/pull/43076

```
rustup update nightly
```

After doing this you'll want to ensure that Cargo's using the nightly toolchain
by invoking it like:

```
cargo +nightly build
```

Next you'll add this to your `Cargo.toml`

```toml
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

## Technical Details

As mentioned before this crate is fundamentally built on the feature of
*generators* in Rust. Generators, otherwise known in this case as stackless
coroutines, allow the compiler to generate an optimal implementation for a
`Future` for a function to transform a synchronous-looking block of code into a
`Future` that can execute asynchronously. The desugaring here is surprisingly
straightforward from code you write to code rustc compiles, and you can browse
the source of the `futures-async-macro` crate for more details here.

Otherwise there's a few primary "APIs" provided by this crate:

* `#[async]` - this attribute can be applied to methods and functions to signify
  that it's an *asynchronous function*. The function's signature *must* return a
  `Result` of some form (although it can return a typedef of results).
  Additionally, **the function's arguments must all be owned values**, or in
  other words must contain no references. This restriction may be lifted in the
  future!

  Some examples are:

  ```rust
  // attribute on a bare function
  #[async]
  fn foo(a: i32) -> Result<u32, String> {
      // ...
  }

  // returning a typedef works too!
  #[async]
  fn foo() -> io::Result<u32> {
      // ...
  }

  impl Foo {
      // methods also work!
      #[async]
      fn foo(self) -> io::Result<u32> {
          // ...
      }
  }

  // For now, however, these do not work, they both have arguments which contain
  // references! This may work one day, but it does not work today.
  //
  // #[async]
  // fn foo(a: &i32) -> io::Result<u32> { /* ... */ }
  //
  // impl Foo {
  //     #[async]
  //     fn foo(&self) -> io::Result<u32> { /* ... */ }
  // }
  ```

  Note that an `#[async]` function is intended to behave very similarly to that
  of its synchronous version. For example the `?` operator works internally, you
  can use an early `return` statement, etc.

  Under the hood an `#[async]` function is compiled to a state machine which
  represents a future for this function, and the state machine's suspension
  points will be where the `await!` macro is located.

* `async_block!` - this macro is similar to `#[async]` in that it creates a
  future, but unlike `#[async]` it can be used in an expression context and
  doesn't need a dedicated function. This macro can be considered as "run this
  block of code asynchronously" where the block of code provided, like an async
  function, returns a result of some form. For example:

  ```rust
  let server = TcpListener::bind(..);
  let future = async_block! {
      #[async]
      for connection in server.incoming() {
          spawn(handle_connection(connection));
      }
      Ok(())
  };
  core.run(future).unwrap();
  ```

  or if you'd like to do some precomputation in a function:

  ```rust
  fn hash_file(path: &Path, pool: &CpuPool)
      -> impl Future<Item = u32, Error = io::Error>
  {
      let abs_path = calculate_abs_path(path);
      async_block! {
          let contents = await!(read_file)?;
          Ok(hash(&contents))
      }
  }
  ```

* `await!` - this is a macro provided in the `futures-await-macro` crate which
  allows waiting on a future to complete. The `await!` macro can only be used
  inside of an `#[async]` function or an `async_block!` and can be thought of as
  a function that looks like:

  ```rust
  fn await!<F: Future>(future: F) -> Result<F::Item, F::Error> {
      // magic
  }
  ```

  Some examples of this macro are:

  ```rust
  #[async]
  fn fetch_url(client: hyper::Client, url: String) -> io::Result<Vec<u8>> {
      // note the trailing `?` to propagate errors
      let response = await!(client.get(url))?;
      await!(response.body().concat())
  }
  ```

* `#[async]` for loops - and finally, the last feature provided by this crate is
  the ability to iterate asynchronously over a `Stream`. You can do this by
  attaching the `#[async]` attribute to a `for` loop where the object being
  iterated over implements the `Stream` trait.

  Errors from the stream will get propagated automatically, and otherwise, the
  for loop will exhauste the stream to completion, binding each element to the
  pattern provided.

  Some examples are:

  ```rust
  #[async]
  fn accept_connections(listener: TcpListener) -> io::Result<()> {
      #[async]
      for connection in server.incoming() {
          // `connection` here has type `TcpStream`
      }
      Ok(())
  }
  ```

  Note that an `#[async]` for loop, like `await!`, can only be used in an async
  function or an async block.

### Nightly features

Right now this crate requires two nightly features to be used, and practically
requires three features to be used to its fullest extent. These three features
are:

* `#![feature(generators)]` - this is an experimental language feature that has
  yet to be stabilized but is the foundation for the implementation of
  async/await. The implemented [landed recently][genpr] and progress on
  stabilization can be found on its [tracking issue][gentrack].

* `#![feature(proc_macro)]` - this has also been dubbed "Macros 2.0" and is how
  the `#[async]` attribute is defined in this crate and not the compiler itself.
  We then also take advantage of other macros 2.0 features like importing macros
  via `use` instead of `#[macro_use]`. Tracking issues for this feature include
  [#38356] and [#35896].

* `#![feature(conservative_impl_trait)]` - this feature is not actually required
  to use the crate if you opt to always use trait objects via `#[async(boxed)]`,
  but it's practically required to use the crate ergonomically. This feature is
  used because each function returns `impl Future<...>` instead of a concrete
  type of future. This feature is tracked at [#34511] and [#42183].

[gentrack]: https://github.com/rust-lang/rust/issues/43122
[#38356]: https://github.com/rust-lang/rust/issues/38356
[#35896]: https://github.com/rust-lang/rust/issues/35896
[#34511]: https://github.com/rust-lang/rust/issues/34511
[#42183]: https://github.com/rust-lang/rust/issues/42183

The intention with this crate is that the newest feature, `generators`, will
be the last to stabilize. The other two, `proc_macro` and
`conservative_impl_trait`, should hopefully stabilize ahead of generators!

## What's next?

This crate is still quite new and generators have only *just* landed on the
nightly channel for Rust. While no major change is planned to this crate at this
time, we'll have to see how this all pans out! One of the major motivational
factors for landing generators so soon in the language was to transitively
empower this implementation of async/await, and your help is needed in assessing
that!

Notably, we're looking for feedback on async/await in this crate itself as well
as generators as a language feature. We want to be sure that if we were to
stabilize generators or procedural macros they're providing the optimal
async/await experience!

At the moment you're encouraged to take caution when using this in production.
This crate itself has only been very lightly tested and the `generators`
language feature has also only been lightly tested so far. Caution is advised
along with a suggestion to keep your eyes peeled for bugs! If you run into any
questions or have any issues, you're more than welcome to [open an
issue][issue]!

[issue]: https://github.com/alexcrichton/futures-await/issues/new

## Caveats

As can be expected with many nightly features, there are a number of caveats to
be aware of when working with this project. Despite this bug reports or
experience reports are more than welcome, always good to know what needs to be
fixed regardless!

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
code like this will not (or at least shouldn't) compile today:

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
function in a trait that wants to return a future. Unfortunately it's actually
quite difficult to use this! Right now there's a few of caveats:

* Ideally you want to tag this `#[async]` but this doesn't work because a trait
  function returning `impl Future` is not implemented in the compiler today. I'm
  told that this will eventually work, though!
* Ok so then the next best thing is `#[async(boxed)]` to return a boxed trait
  object instead of `impl Future` for the meantime. This may not be quite what
  we want runtime-wise but we also have problems with...
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
    #[async]
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
    #[async]
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

You can emulate this today with a non-object-safe implementation via:

```rust
trait Foo {
    #[async]
    fn do_async_task(me: Rc<Self>) -> Result<i32, u32>;
}

fn foo<T: Foo>(t: Rc<T>) {
    let x = Foo::trait_fn(t);
    // ...
}
```

But that's not exactly the most ergonomic!

#### Associated types

Another limitation when using futures with traits is with associated types.
Say you've got a trait like:

```rust
trait Service {
    type Request;
    type Error;
    type Future: Future<Item = Self::Response, Error = Self::Error>;
    
    fn call(&self) -> Self::Future;
}
```

If you want to implement `call` with `async_block!`, or by returning a future
from another function which was generated with `#[async]`, you'd probably want
to use `impl Future`. Unfortunately, it's not current possible to express an
associated constant like `Service::Future` with an `impl Trait`.

For now the best solution is to use `Box<Future<...>>`:

```rust
impl Service for MyStruct {
    type Request = ...;
    type Error = ...;
    type Future = Box<Future<Item = Self::Item, Error = Self::Error>>;
    
    fn call(&self) -> Self::Future {
        // ...
        Box::new(future)
    }
}
```

# License

`futures-await` is primarily distributed under the terms of both the MIT
license and the Apache License (Version 2.0), with portions covered by various
BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
