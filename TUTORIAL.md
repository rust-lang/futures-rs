# Getting started with futures
[top]: #getting-started-with-futures

This document will help you learn about [`futures`], a Rust crate with a
zero-cost implementation of [futures] and streams. Futures are available in many
other languages like [C++][cpp-futures], [Java][java-futures], and
[Scala][scala-futures], and this crate draws inspiration from these
libraries. The `futures` crate, however, distinguishes itself by being
both ergonomic as well as adhering to the Rust philosophy of zero-cost
abstractions. More concretely, futures do not require allocations to
create and compose, and the per-connection `Task` that drives futures requires
only one. Futures are intended to be the foundation for asynchronous,
composable, high performance I/O in Rust, and [early benchmarks] show that a
simple HTTP server built on futures is really fast!

[`futures`]: https://github.com/alexcrichton/futures-rs
[futures]: https://en.wikipedia.org/wiki/Futures_and_promises
[java-futures]: https://docs.oracle.com/javase/7/docs/api/java/util/concurrent/Future.html
[cpp-futures]: http://en.cppreference.com/w/cpp/thread/future
[scala-futures]: http://docs.scala-lang.org/overviews/core/futures.html
[early benchmarks]: https://github.com/alexcrichton/futures-rs/tree/master/futures-minihttp#futures-minihttp

This document is split up into a few sections:

* [Hello, World!][hello-world]
* [The `Future` trait][future-trait]
* [The `Stream` trait][stream-trait]
* [Concrete futures and stream][concrete]
* [Returning futures][returning-futures]
* [`Task` and `Future`][task-and-future]
* [Task local data][task-local-data]
* [Event loop data][event-loop-data]

If you'd like to help contribute to this document you can find it on
[GitHub][online-doc].

[online-doc]: https://github.com/alexcrichton/futures-rs/blob/master/TUTORIAL.md

---

## Hello, World!
[hello-world]: #hello-world

[Back to top][top]

The `futures` crate requires Rust 1.9.0 or greater, which can be easily
obtained through [rustup]. Windows, macOS, and Linux are all tested and known to
work, but PRs for other platforms are always welcome! You can add
futures to your project's `Cargo.toml` like so:

[rustup]: https://www.rustup.rs/

```toml
[dependencies]
futures = { git = "https://github.com/alexcrichton/futures-rs" }
futures-io = { git = "https://github.com/alexcrichton/futures-rs" }
futures-mio = { git = "https://github.com/alexcrichton/futures-rs" }
futures-tls = { git = "https://github.com/alexcrichton/futures-rs" }
```

> **Note**: this library is currently in active development and requires
>           pulling from git right now, but soon the crates will be published to
>           crates.io!

Here we're adding a dependency on three crates:

* [`futures`] - the definition and core implementation of [`Future`] and
  [`Stream`].
* [`futures-io`] - I/O abstractions built with these two traits.
* [`futures-mio`] - bindings to the [`mio`] crate providing concrete
  implementations of [`Future`], [`Stream`], and [`futures-io`] abstractions
  with TCP and UDP.
* [`futures-tls`] - an SSL/TLS implementation built on top of [`futures-io`].

[`mio`]: https://crates.io/crates/mio
[`futures-io`]: https://github.com/alexcrichton/futures-rs/tree/master/futures-io
[`futures-mio`]: https://github.com/alexcrichton/futures-rs/tree/master/futures-mio
[`futures-tls`]: https://github.com/alexcrichton/futures-rs/tree/master/futures-tls
[`Future`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html
[`Stream`]: http://alexcrichton.com/futures-rs/futures/stream/trait.Stream.html

The [`futures`] crate itself is a low-level implementation of futures which does
not assume any particular runtime or I/O layer. For the examples below we'll be
using the concrete implementations available in [`futures-mio`] and
[`futures-io`] to show how futures and streams can be used to perform
sophisticated I/O with zero abstraction overhead.

Now that we've got all that set up, let's write our first program! As
a "Hello, World!" for I/O, let's download the Rust home page:

```rust
extern crate futures;
extern crate futures_io;
extern crate futures_mio;
extern crate futures_tls;

use std::net::ToSocketAddrs;

use futures::Future;
use futures_mio::Loop;
use futures_tls::ClientContext;

fn main() {
    let mut lp = Loop::new().unwrap();
    let addr = "www.rust-lang.org:443".to_socket_addrs().unwrap().next().unwrap();

    let socket = lp.handle().tcp_connect(&addr);

    let tls_handshake = socket.and_then(|socket| {
        let cx = ClientContext::new().unwrap();
        cx.handshake("www.rust-lang.org", socket)
    });
    let request = tls_handshake.and_then(|socket| {
        futures_io::write_all(socket, "\
            GET / HTTP/1.0\r\n\
            Host: www.rust-lang.org\r\n\
            \r\n\
        ".as_bytes())
    });
    let response = request.and_then(|(socket, _)| {
        futures_io::read_to_end(socket, Vec::new())
    });

    let data = lp.run(response).unwrap();
    println!("{}", String::from_utf8_lossy(&data));
}
```

If you place that file in `src/main.rs`, and then execute `cargo run`, you
should see the HTML of the Rust home page!

> Note: `rustc` 1.10 compiles this example slowly. With 1.11 it builds
considerably faster.

There's a lot to digest here, though, so let's walk through it
line-by-line. First up in `main()`:

```rust
let mut lp = Loop::new().unwrap();
let addr = "www.rust-lang.org:443".to_socket_addrs().unwrap().next().unwrap();
```

Here we [create an event loop][loop-new] on which we will perform all our
I/O. Then we resolve the "www.rust-lang.org" host name by using
the standard library's [`to_socket_addrs`] method.

[loop-new]: http://alexcrichton.com/futures-rs/futures_mio/struct.Loop.html#method.new
[`to_socket_addrs`]: https://doc.rust-lang.org/std/net/trait.ToSocketAddrs.html

Next up:

```rust
let socket = lp.handle().tcp_connect(&addr);
```

We [get a handle] to our event loop and connect to the host with
[`tcp_connect`]. Note, though, that [`tcp_connect`] returns a future! This
means that we don't actually have the socket yet, but rather it will
be fully connected at some later point in time.

[get a handle]: http://alexcrichton.com/futures-rs/futures_mio/struct.Loop.html#method.handle
[`tcp_connect`]: http://alexcrichton.com/futures-rs/futures_mio/struct.LoopHandle.html#method.tcp_connect

Once our socket is available we need to perform three tasks to download the
rust-lang.org home page:

1. Perform a TLS handshake. The home page is only served over HTTPS, so we had
   to connect to port 443 and we'll have to obey the TLS protocol.
2. An HTTP 'GET' request needs to be issued. For the purposes of this tutorial
   we will write the request by hand, though in a serious program you would
   use an HTTP client built on futures.
3. Finally, we download the response by reading off all the data on the socket.

Let's take a look at each of these steps in detail, the first being:

```rust
let tls_handshake = socket.and_then(|socket| {
    let cx = ClientContext::new().unwrap();
    cx.handshake("www.rust-lang.org", socket)
});
```

Here we use the [`and_then`] method on the [`Future`] trait to continue building
on the future returned by [`tcp_connect`]. The [`and_then`] method takes a
closure which receives the resolved value of this previous future. In this case
`socket` will have type [`TcpStream`]. The [`and_then`] closure, however, will
not run if [`tcp_connect`] returned an error.

[`and_then`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#method.and_then
[`TcpStream`]: http://alexcrichton.com/futures-rs/futures_mio/struct.TcpStream.html

Once we have our `socket`, we create a client TLS context via
[`ClientContext::new`]. This type from the [`futures-tls`] crate
represents the client half of a TLS connection. Next we call the
[`handshake`] method to actually perform the TLS handshake. The first
argument is the domain name we're connecting to, with the I/O object
as the second.

[`ClientContext::new`]: http://alexcrichton.com/futures-rs/futures_tls/struct.ClientContext.html#method.new
[`handshake`]: http://alexcrichton.com/futures-rs/futures_tls/struct.ClientContext.html#method.handshake

Like with [`tcp_connect`] from before, the [`handshake`] method
returns a future. The actual TLS handshake may take some time as the
client and server need to perform some I/O, agree on certificates,
etc. Once resolved, however, the future will become a [`TlsStream`],
similar to our previous [`TcpStream`]

[`TlsStream`]: http://alexcrichton.com/futures-rs/futures_tls/struct.TlsStream.html

The [`and_then`] combinator is doing some heavy lifting behind the
scenes here by ensuring that it executes futures in the right order
and keeping track of the futures in flight. Even better, the value
returned from [`and_then`] itself implements [`Future`], so we can
keep chaining computation!

Next up, we issue our HTTP request:

```rust
let request = tls_handshake.and_then(|socket| {
    futures_io::write_all(socket, "\
        GET / HTTP/1.0\r\n\
        Host: www.rust-lang.org\r\n\
        \r\n\
    ".as_bytes())
});
```

Here we take the future from the previous step, `tls_handshake`, and
use [`and_then`] again to continue the computation. The [`write_all`]
combinator writes the entirety of our HTTP request, issueing multiple
writes as necessary. Here we're just doing a simple HTTP/1.0 request,
so there's not much we need to write.

[`write_all`]: http://alexcrichton.com/futures-rs/futures_io/fn.write_all.html

The future returned by [`write_all`] will complete once all the data
has been written to the socket. Note that behind the scenes the
[`TlsStream`] will actually be encrypting all the data we write before
sending it to the underlying socket.

And the third and final piece of our request looks like:

```rust
let response = request.and_then(|(socket, _)| {
    futures_io::read_to_end(socket, Vec::new())
});
```

The previous `request` future is chained again to the final future,
the [`read_to_end`] combinator. This future will read all data from the
`socket` provided and place it into the buffer provided (in this case an empty
one), and resolve to the buffer itself once the underlying connection hits EOF.

[`read_to_end`]: http://alexcrichton.com/futures-rs/futures_io/fn.read_to_end.html

Like before, though, reads from the `socket` are actually decrypting data
received from the server under the covers, so we're just reading the decrypted
version!

If we were to return at this point in the program, you might be surprised to see
that nothing happens when it's run! That's because all we've done so
far is construct a future-based computation, we haven't actually run it. Up to
this point in the program we've done no I/O, issued no HTTP requests, etc.

To actually execute our future and drive it to completion we'll need to run the
event loop:

```rust
let data = lp.run(response).unwrap();
println!("{}", String::from_utf8_lossy(&data));
```

Here we pass our `response` future, our entire HTTP request, and we pass it to
the event loop, [asking it to resolve the future][`run`]. The event loop will
then run until the future has been resolved, returning the result of the future
which in this case is `io::Result<Vec<u8>>`.

[`run`]: http://alexcrichton.com/futures-rs/futures_mio/struct.Loop.html#method.run

Note that this `lp.run(..)` call will block the calling thread until the future
can itself be resolved. This means that `data` here has type `Vec<u8>`. We then
print it out to stdout as usual.

Phew! At this point we've seen futures [initiate a TCP connection][`tcp_connect`]
[create a chain of computation][`and_then`], and [read data from a
socket][`read_to_end`]. But this is only a hint of what futures can
do, so let's dive more into the traits themselves!

---

## The `Future` trait
[future-trait]: #the-future-trait

[Back to top][top]

The core trait of the [`futures`] crate is [`Future`].  This trait represents an
asynchronous computation which will eventually get resolved. Let's take a look:

```rust
trait Future: Send + 'static {
    type Item: Send + 'static;
    type Error: Send + 'static;

    fn poll(&mut self, task: &mut Task) -> Poll<Self::Item, Self::Error>;
    fn schedule(&mut self, task: &mut Task);

    // ...
}
```

I'm sure quite a few points jump out immediately about this definition, so
let's go through them all in detail!

* [`Send` and `'static`][send-and-static]
* [`Item` and `Error`][item-and-error]
* [`poll` and `schedule`][poll-and-schedule]
* [`Future` combinators][combinators]

### `Send` and `'static`
[send-and-static]: #send-and-static

[Back to `Future`][future-trait]

```rust
trait Future: Send + 'static {
    // ...
}
```

The first part of the `Future` trait you'll probably notice are the `Send +
'static` bounds. This is Rust's way of saying that a type can be sent to other
threads and also contains no stack references. This restriction on `Future` as
well as its associated types provides the guarantee that all futures, and their
results, can be sent to other threads.

These bounds on the trait definition make futures maximally useful in
multithreaded scenarios - one of Rust's core strengths - with the
tradeoff that implementing futures for _non-`Send`_ data is not
straightforward (though it is possible). The `futures` crate makes
this tradeoff to empower consumers of futures, at the expense of
implementors of futures, as consumers are by far the more common case.
As a bonus, these bounds allow for some [interesting
optimizations][tailcall].

[tailcall]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#method.tailcall

For more discussion on futures of non-`Send` data see the section on [event loop
data][event-loop-data]. Additionally, more technical information about these
bounds can be found [in the FAQ][faq-why-send].

[faq-why-send]: https://github.com/alexcrichton/futures-rs/blob/master/FAQ.md#why-send--static

### `Item` and `Error`
[item-and-error]: #send-and-static

[Back to `Future`][future-trait]

```rust
type Item: Send + 'static;
type Error: Send + 'static;
```

The next aspect of the [`Future`] trait you'll probably notice is the two
associated types it contains. These represent the types of values that the
`Future` can resolve to. Each instance of `Future` can be thought of as
resolving to a `Result<Self::Item, Self::Error>`.

Each associated type, like the trait, is bound by `Send + 'static`, indicating
that they must be sendable to other threads and cannot contain stack references.

These two types will show up very frequently in `where` clauses when consuming
futures generically, and type signatures when futures are returned. For example
when returning a future you might write:

```rust
fn foo() -> Box<Future<Item = u32, Error = io::Error>> {
    // ...
}
```

Or when taking a future you might write:

```rust
fn foo<F>(future: F)
    where F: Future<Error = io::Error>,
          F::Item: u32,
{
    // ...
}
```

### `poll` and `schedule`
[poll-and-schedule]: #poll-and-schedule

[Back to `Future`][future-trait]

```rust
fn poll(&mut self, task: &mut Task) -> Poll<Self::Item, Self::Error>;
fn schedule(&mut self, task: &mut Task);
```

The entire [`Future`] trait is built up around these two methods, and they are
the only required methods. The [`poll`] method is the sole entry point for
extracting the resolved value of a future, and the [`schedule`] method is how
interest is registered in the value of a future, should it become available.
As a consumer of futures you will rarely - if ever - need to call these methods
directly. Rather, you interact with futures through [combinators] that create
higher-level abstractions around futures. But it's useful to our understanding
if we have a sense of how futures work under the hood.

[`poll`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#tymethod.poll
[`schedule`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#tymethod.schedule

These two methods are similar to [`epoll`] or [`kqueue`] in that they embody a
*readiness* model of computation rather than a *completion*-based model. That
is, futures implementations notify their consumers that data is _ready_ through
the [`schedule`] method, and then values are extracted through the nonblocking
[`poll`] method. If [`poll`] is called and the value isn't ready yet then the
`schedule` method can be invoked to learn about when the value is itself
available.

[`epoll`]: http://man7.org/linux/man-pages/man7/epoll.7.html
[`kqueue`]: https://www.freebsd.org/cgi/man.cgi?query=kqueue&sektion=2

First, let's take a closer look at [`poll`]. Notice the
`&mut self` argument, which conveys a number of restrictions and abilities:

* Futures may only be polled by one thread at a time.
* During a `poll`, futures can mutate their own state.
* When `poll`'d, futures are owned by another entity.

Next up is the [`Task`] argument, but we'll [talk about that
later][task-and-future]. Finally we see the [`Poll`][poll-type] type being returned,
which looks like.

[`Task`]: http://alexcrichton.com/futures-rs/futures/struct.Task.html
[poll-type]: http://alexcrichton.com/futures-rs/futures/enum.Poll.html

```rust
enum Poll<T, E> {
    NotReady,
    Ok(T),
    Err(E),
}
```

Through this `enum` futures can communicate whether the future's value is ready
to go via `Poll::Ok` or `Poll::Err`. If the value isn't ready yet then
`Poll::NotReady` is returned.

The [`Future`] trait, like `Iterator`, doesn't specify what happens after
[`poll`] is called if the future has already resolved. Many implementations will
panic, some may never resolve again, etc. This means that implementors of the
[`Future`] trait don't need to maintain state to check if [`poll`] has already
returned successfully.

If a call to [`poll`] returns `Poll::NotReady`, then futures still need to know
how to figure out when to get poll'd later! This is where the [`schedule`]
method comes into the picture. Like with [`poll`], this method takes `&mut self`,
giving us the same guarantees as before. Similarly, it is passed a [`Task`], but
the relationship between [`schedule`] and [`Task`] is somewhat different than
that `poll` has.

Each [`Task`] can have a [`TaskHandle`] extracted from it via the
[`Task::handle`] method. This [`TaskHandle`] implements `Send + 'static` and has
one primary method, [`notify`]. This method, when called, indicates that a
future can make progress, and may be able to resolve to a value.

[`Task::handle`]: http://alexcrichton.com/futures-rs/futures/struct.Task.html#method.handle
[`TaskHandle`]: http://alexcrichton.com/futures-rs/futures/struct.TaskHandle.html
[`notify`]: http://alexcrichton.com/futures-rs/futures/struct.TaskHandle.html#method.notify

That is, when [`schedule`] is called, a future must arrange for the [`Task`]
specified to get notified when progress is ready to be made. It's ok to notify a
task when the future can't actually get resolved, or just for a spurious
notification that something is ready. Nevertheless, implementors of [`Future`]
must guarantee that after the value is ready at least one call to `notify` is
made.

More detailed documentation can be found on the [`poll`] and [`schedule`]
methods themselves.

### `Future` combinators
[combinators]: #future-combinators

[Back to `Future`][future-trait]

Now that we've seen the [`poll`] and [`schedule`] methods, they seem like they
may be a bit of a pain to call! What if all you have is a future of `String` and
you want to convert it to a future of `u32`? For this sort of composition, the
`Future` trait also provides a large number of **combinators** which can be seen
on the [`Future`] trait itself.

These combinators similar to the [`Iterator`] combinators in that they all
consume the receiving future and return a new future. For example, we could
have:

```rust
fn parse<F>(future: F) -> Box<Future<Item=u32, Error=F::Error>>
    where F: Future<Item=String>,
{
    future.map(|string| {
        string.parse::<u32>().unwrap()
    }).boxed()
}
```

Here we're using [`map`] to transform a future of `String` to a future of `u32`,
ignoring errors. This example returns a [`Box`], but that's not always necessary,
and is discussed in the [returning futures][returning-futures] section.

The combinators on futures allow expressing concepts like:

* Change the type of a future ([`map`], [`map_err`])
* Run another future after one has completed ([`then`], [`and_then`],
  [`or_else`])
* Figuring out which of two futures resolves first ([`select`])
* Waiting for two futures to both complete ([`join`])
* Defining the behavior of [`poll`] after resolution ([`fuse`])

[`Iterator`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html
[`Box`]: https://doc.rust-lang.org/std/boxed/struct.Box.html
[`map`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#method.map
[`map_err`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#method.map_err
[`then`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#method.then
[`and_then`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#method.and_then
[`or_else`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#method.or_else
[`select`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#method.select
[`join`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#method.join
[`fuse`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#method.fuse

Usage of the combinators should feel very similar to the [`Iterator`] trait in
Rust or [futures in Scala][scala-futures]. Most composition of futures ends up
being done through these combinators. All combinators are zero-cost, that means
no memory is allocated internally and the implementation will optimize to what
you would have otherwise written by hand.

---

## The `Stream` trait
[stream-trait]: #the-stream-trait

[Back to top][top]

Previously, we've taken a long look at the [`Future`] trait which is useful if
we're only producing one value over time. But sometimes computations are best
modeled as a *stream* of values being produced over time. For example, a TCP
listener produces a number of TCP socket connections over its lifetime.
Let's see how [`Future`] and [`Stream`] relate to their synchronous equivalents
in the standard library:

| # items | Sync | Async      | Common operations                              |
| ----- | -----  | ---------- | ---------------------------------------------- |
| 1 | [`Result`]   | [`Future`] | [`map`], [`and_then`]                        |
| ∞ | [`Iterator`] | [`Stream`] | [`map`][stream-map], [`fold`], [`collect`]   |

[`Result`]: https://doc.rust-lang.org/std/result/enum.Result.html

Let's take a look at the [`Stream`] trait in the [`futures`] crate:

```rust
trait Stream: Send + 'static {
    type Item: Send + 'static;
    type Error: Send + 'static;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Self::Item>, Self::Error>;
    fn schedule(&mut self, task: &mut Task);
}
```

You'll notice that the [`Stream`] trait is very similar to the [`Future`] trait.
It requires `Send + 'static`, has associated types for the item/error, and has a
`poll` and `schedule` method. The primary difference, however, is that a
stream's [`poll`][stream-poll] method returns `Option<Self::Item>` instead of
`Self::Item`.

[stream-poll]: http://alexcrichton.com/futures-rs/futures/stream/trait.Stream.html#tymethod.poll

A [`Stream`] produces optionally many values over time, signaling termination of
the stream by returning `Poll::Ok(None)`. At its heart a [`Stream`] represents
an asynchronous stream of values being produced in order.

A [`Stream`] is actually just a special instance of a [`Future`], and can be
converted to a future through the [`into_future`] method. The [returned
future][`StreamFuture`] will resolve to the next value on the stream plus the
stream itself, allowing more values to later be extracted. This also allows
composing streams and other arbitrary futures with the core future combinators.

[`into_future`]: http://alexcrichton.com/futures-rs/futures/stream/trait.Stream.html#method.into_future
[`StreamFuture`]: http://alexcrichton.com/futures-rs/futures/stream/struct.StreamFuture.html

Like [`Future`], the [`Stream`] trait provides a large number of combinators.
Many future-like combinators are provided, like [`then`][stream-then], in
addition to stream-specific combinators like [`fold`].

[stream-then]: http://alexcrichton.com/futures-rs/futures/stream/trait.Stream.html#method.then
[stream-map]: http://alexcrichton.com/futures-rs/futures/stream/trait.Stream.html#method.map
[`fold`]: http://alexcrichton.com/futures-rs/futures/stream/trait.Stream.html#method.fold
[`collect`]: http://alexcrichton.com/futures-rs/futures/stream/trait.Stream.html#method.collect

### `Stream` Example
[stream-example]: #stream-example

[Back to `Stream`][stream-trait]

We saw an example of using futures at the beginning of this tutorial, so let's
take a look at an example of streams now, the [`incoming`] implementation of
[`Stream`] on [`TcpListener`]. The simple server below will accept connections,
write out the word "Hello!" to them, and then close the socket:

[`incoming`]: http://alexcrichton.com/futures-rs/futures_mio/struct.TcpListener.html#method.incoming
[`TcpListener`]: http://alexcrichton.com/futures-rs/futures_mio/struct.TcpListener.html

```rust
extern crate futures;
extern crate futures_io;
extern crate futures_mio;

use futures::Future;
use futures::stream::Stream;
use futures_mio::Loop;

fn main() {
    let mut lp = Loop::new().unwrap();
    let address = "127.0.0.1:8080".parse().unwrap();
    let listener = lp.handle().tcp_listen(&address);

    let server = listener.and_then(|listener| {
        let addr = listener.local_addr().unwrap();
        println!("Listening for connections on {}", addr);

        let clients = listener.incoming();
        let welcomes = clients.and_then(|(socket, _peer_addr)| {
            futures_io::write_all(socket, b"Hello!\n")
        });
        welcomes.for_each(|(_socket, _welcome)| {
            Ok(())
        })
    });

    lp.run(server).unwrap();
}
```

Like before, let's walk through this line-by-line:

```rust
let mut lp = Loop::new().unwrap();
let address = "127.0.0.1:8080".parse().unwrap();
let listener = lp.handle().tcp_listen(&address);
```

Here we initialize our event loop, like before, and then we use the
[`tcp_listen`] method on [`LoopHandle`] to create a TCP listener which will
accept sockets.

[`tcp_listen`]: http://alexcrichton.com/futures-rs/futures_mio/struct.LoopHandle.html#method.tcp_listen

Next up we see:

```rust
let server = listener.and_then(|listener| {
    // ...
});
```

Here we see that [`tcp_listen`], like [`tcp_connect`] from before, did not
return a [`TcpListener`] but rather a future which will resolve to a TCP
listener. We then employ the [`and_then`] method on [`Future`] to define what
happens once the TCP listener is available.

Now that we've got the TCP listener we can inspect its state:

```rust
let addr = listener.local_addr().unwrap();
println!("Listening for connections on {}", addr);
```

Here we're just calling the [`local_addr`] method to print out what address we
ended up binding to. We know that at this point the port is actually bound
successfully, so clients can now connect.

[`local_addr`]: http://alexcrichton.com/futures-rs/futures_mio/struct.TcpListener.html#method.local_addr

Next up, we actually create our [`Stream`]!

```rust
let clients = listener.incoming();
```

Here the [`incoming`] method returns a [`Stream`] of [`TcpListener`] and
[`SocketAddr`] pairs. This is similar to [libstd's `TcpListener`] and the
[`accept` method][accept], only we're receiving all of the events as a stream rather
than having to manually accept sockets.

The stream, `clients`, produces sockets forever. This mirrors how socket servers
tend to accept clients in a loop and then dispatch them to the rest of the
system for processing.

[libstd's `TcpListener`]: https://doc.rust-lang.org/std/net/struct.TcpListener.html
[`SocketAddr`]: https://doc.rust-lang.org/std/net/enum.SocketAddr.html
[accept]: https://doc.rust-lang.org/std/net/struct.TcpListener.html#method.accept

Now that we've got our stream of clients, we can manipulate it via the standard
methods on the [`Stream`] trait:

```rust
let welcomes = clients.and_then(|(socket, _peer_addr)| {
    futures_io::write_all(socket, b"Hello!\n")
});
```

Here we use the [`and_then`][stream-and-then] method on [`Stream`] to perform an
action over each item of the stream. In this case we're chaining on a
computation for each element of the stream (in this case a `TcpStream`). The
computation is the same [`write_all`] we saw earlier, where it'll write the
entire buffer to the socket provided.

[stream-and-then]: http://alexcrichton.com/futures-rs/futures/stream/trait.Stream.html#method.and_then

This block means that `welcomes` is now a stream of sockets which have had
"Hello!" written to them. For our purposes we're done with the connection at
that point, so we can collapse the entire `welcomes` stream into a future with
the [`for_each`] method:

```rust
welcomes.for_each(|(_socket, _welcome)| {
    Ok(())
})
```

Here we take the results of the previous future, [`write_all`], and discard
them, closing the socket.

[`for_each`]: http://alexcrichton.com/futures-rs/futures/stream/trait.Stream.html#method.for_each

Note that an important limitation of this server is that there is *no concurrency*!
Streams represent in-order processing of data, and in this case the order of the
original stream is the order in which sockets are received, which the
[`and_then`][stream-and-then] and [`for_each`] combinators preserve.

If, instead, we want to handle all clients concurrently, we can use the
[`forget`] method:

```rust
let clients = listener.incoming();
let welcomes = clients.map(|(socket, _peer_addr)| {
    futures_io::write_all(socket, b"Hello!\n")
});
welcomes.for_each(|future| {
    future.forget();
    Ok(())
})
```

Instead of [`and_then`][stream-and-then] we're using [`map`][stream-map] here
which changes our stream of clients to a stream of futures. We then change our
[`for_each`] closure to *[`forget`]* the future, which allows the future to
execute concurrently.

---

## Concrete futures and streams
[concrete]: #concrete-futures-and-streams

[Back to top][top]

Alright! At this point we've got a good understanding of the [`Future`] and [`Stream`]
traits, both how they're implemented as well as how they're composed together.
But where do all these futures originally come from? Let's take a look at a few
concrete implementations of futures and streams.

First, any value already available is trivially a future that is immediately
ready. For this, the [`done`], [`failed`], [`finished`] functions suffice. The
[`done`] variant takes a `Result<T, E>` and returns a `Future<Item=T, Error=E>`.
The [`failed`] and [`finished`] variants then specify either `T` or `E` and
leave the other associated type as a wildcard.

[`done`]: http://alexcrichton.com/futures-rs/futures/fn.done.html
[`finished`]: http://alexcrichton.com/futures-rs/futures/fn.finished.html
[`failed`]: http://alexcrichton.com/futures-rs/futures/fn.failed.html

For streams, the equivalent of an "immediately ready" stream is the [`iter`]
function which creates a stream that yields the same items as the underlying
iterator.

[`iter`]: http://alexcrichton.com/futures-rs/futures/stream/fn.iter.html

In situations though where a value isn't immediately ready, there are also
more general implementations of [`Future`] and [`Stream`] that are available in
the [`futures`] crate, the first of which is [`promise`]. Let's take a look:

[`promise`]: http://alexcrichton.com/futures-rs/futures/fn.promise.html

```rust
extern crate futures;

use std::thread;
use futures::Future;

fn expensive_computation() -> u32 {
    // ...
    200
}

fn main() {
    let (tx, rx) = futures::promise();

    thread::spawn(move || {
        tx.complete(expensive_computation());
    });

    let rx = rx.map(|x| x + 3);
}
```

Here we can see that the [`promise`] function returns two halves (like
[`mpsc::channel`]). The first half, `tx` ("transmitter"), is of type [`Complete`]
and is used to complete the promise, providing a value to the future on the
other end. The [`Complete::complete`] method will transmit the value to the
receiving end.

The second half, `rx` ("receiver"), is of type [`Promise`][promise-type] which is
a type that implements the [`Future`] trait. The `Item` type is `T`, the type of 
the promise.
The `Error` type is [`Canceled`], which happens when the [`Complete`] half is
dropped without completing the computation.

[`mpsc::channel`]: https://doc.rust-lang.org/std/sync/mpsc/fn.channel.html
[`Complete`]: http://alexcrichton.com/futures-rs/futures/struct.Complete.html
[`Complete::complete`]: http://alexcrichton.com/futures-rs/futures/struct.Complete.html#method.complete
[promise-type]: http://alexcrichton.com/futures-rs/futures/struct.Promise.html
[`Canceled`]: http://alexcrichton.com/futures-rs/futures/struct.Canceled.html

This concrete implementation of `Future` can be used (as shown here) to
communicate values across threads. Each half implements the `Send` trait and is
a separately owned entity to get passed around. It's generally not recommended
to make liberal use of this type of future, however; the combinators above or
other forms of base futures should be preferred wherever possible.

For the [`Stream`] trait, a similar primitive is available, [`channel`]. This
type also has two halves, where the sending half is used to send messages and
the receiving half implements `Stream`.

The channel's [`Sender`] type differs from the standard library's in an
important way: when a value is sent to the channel it consumes the sender,
returning a future that will resolve to the original sender only once the sent
value is consumed. This creates backpressure so that a producer won't be able to
make progress until the consumer has caught up.

[`channel`]: http://alexcrichton.com/futures-rs/futures/stream/fn.channel.html
[`Sender`]: http://alexcrichton.com/futures-rs/futures/stream/struct.Sender.html

---

## Returning futures
[returning-futures]: #returning-futures

[Back to top][top]

When working with futures, one of the first things you're likely to need to do
is to return a [`Future`]! Like with the [`Iterator`] trait, however, this
isn't (yet) the easiest thing to do. Let's walk through your options:

* [Trait objects][return-trait-objects]
* [Custom types][return-custom-types]
* [Named types][return-named-types]
* [`impl Trait`][return-impl-trait]

### Trait objects
[return-trait-objects]: #trait-objects

[Back to returning future][returning-futures]

First, what you can do is return a boxed [trait object]:

```rust
fn foo() -> Box<Future<Item = u32, Error = io::Error>> {
    // ...
}
```

The upside of this strategy is that it's easy to write down (just a [`Box`]) and
easy to create (through the [`boxed`] method). This is also maximally flexible
in terms of future changes to the method as *any* type of future can be returned
as an opaque, boxed `Future`.

The downside of this approach is that it requires a runtime allocation
when the future is constructed. The `Box` needs to be allocated on the heap and
the future itself is then placed inside. Note, though that this is the *only*
allocation here, otherwise while the future is being executed no allocations
will be made. Furthermore, the cost is not always that high in the end because
internally there are no boxed futures (i.e. chains of combinators do not
generally introduce allocations), it's only at the fringe that a `Box` comes
into effect.

[trait object]: https://doc.rust-lang.org/book/trait-objects.html
[`boxed`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#method.boxed

### Custom types
[return-custom-types]: #custom-types

[Back to returning future][returning-futures]

If you'd like to not return a `Box`, however, another option is to name the
return type explicitly. For example:

```rust
struct MyFuture {
    inner: Promise<i32>,
}

fn foo() -> MyFuture {
    let (tx, rx) = promise();
    // ...
    MyFuture { inner: tx }
}

impl Future for MyFuture {
    // ...
}
```

In this example we're returning a custom type, `MyFuture`, and we implement the
`Future` trait directly for it. This implementation leverages an underlying
`Promise<i32>`, but any other kind of protocol can also be implemented here as
well.

The upside to this approach is that it won't require a `Box` allocation and it's
still maximally flexible. The implementation details of `MyFuture` are hidden to
the outside world so it can change without breaking others.

The downside to this approach, however, is that it's not always ergonomically
viable. Defining new types gets cumbersome after awhile, and if you're very
frequently returning futures it may be too much.

### Named types
[return-named-types]: #named-types

[Back to returning future][returning-futures]

The next possible alternative is to name the return type directly:

```rust
fn add_10<F>(f: F) -> Map<F, fn(i32) -> i32>
    where F: Future<Item = i32>,
{
    fn do_map(i: i32) -> i32 { i + 10 }
    f.map(do_map)
}
```

Here we name the return type exactly as the compiler sees it. The [`map`]
function returns the [`Map`] struct which internally contains the future and the
function to perform the map.

The upside to this approach is that it's more ergonomic than the custom future
type above and it also doesn't have the runtime overhead of `Box` from before.

The downside, however, is that it's often quite difficult to name the type.
Sometimes the types can get quite large or be unnameable altogether. Here we're
using a function pointer (`fn(i32) -> i32`) but we would ideally use a closure.
Unfortunately the return type cannot name the closure, for now.

[`Map`]: http://alexcrichton.com/futures-rs/futures/struct.Map.html

### `impl Trait`
[return-impl-trait]: #impl-trait

[Back to returning future][returning-futures]

In an ideal world, however, we can have our cake and eat it too with a new
language feature called [`impl Trait`]. This language feature will allow, for
example:

[`impl Trait`]: https://github.com/rust-lang/rfcs/blob/master/text/1522-conservative-impl-trait.md

```rust
fn add_10<F>(f: F) -> impl Future<Item = i32, Error = F::Error>
    where F: Future<Item = i32>,
{
    f.map(|i| i + 10)
}
```

Here we're indicating that the return type is "something that implements
`Future`" with the given associated types. Other than that we just use the
future combinators as we normally would.

The upsides to this approach are that it is zero overhead with no `Box`
necessary, it's maximally flexible to future implementations as the actual
return type is hidden, and it's ergonomic to write as it's similar to the nice
`Box` example above.

The downside to this approach is only that it's not on stable Rust yet. As of
the time of this writing [`impl Trait`] has an [initial implementation as a
PR][impl-trait-pr] but it will still take some time to make its way into nightly
and then finally the stable channel. The good news, however, is that as soon as
`impl Trait` hits stable Rust all crates using futures can immediately benefit!
It should be a backwards-compatible extension to change return types
from `Box` to [`impl Trait`]

[impl-trait-pr]: https://github.com/rust-lang/rust/pull/35091

---

## `Task` and `Future`
[task-and-future]: #task-and-future

[Back to top][top]

Up to this point we've talked a lot about how to build computations by creating
futures, but we've barely touched on how to actually *run* a future. When
talking about [`poll` and `schedule`][poll-and-schedule] earlier it was
mentioned that if `poll` returns `NotReady` then `schedule` is called, but who's
actually calling `poll` and `schedule`?

Enter, a [`Task`]!

In the [`futures`] crate the [`Task`] struct
drives a computation represented by futures. Any particular instance of a
`Future` may be short-lived, only a part of a larger computation. For
example, in our ["hello world"][hello-world] example we had a number of futures,
but only one actually ran at a time. For the entire program, we
had one [`Task`] that followed the logical "thread of execution" as each
future resolved and the overall computation progressed.

In short, a `Task` is the entity that actually orchestrates the top-level calls
to `poll` and `schedule`. Its main method, [`run`], does exactly this. Internally,
`Task` has synchronization such that if [`notify`] is called on multiple threads
then the resulting calls to [`poll`] are properly coordinated.

[`run`]: http://alexcrichton.com/futures-rs/futures/struct.Task.html#method.run

Tasks themselves are not typically created manually but rather are manufactured
through use of the [`forget`] function. This function on the [`Future`] trait
creates a new [`Task`] and then asks it to run the future, resolving the entire
chain of composed futures in sequence.

_The clever implementation of `Task` is the key to the `futures` crate's
efficiency: when a `Task` is created, each of the `Future`s in the chain of
computations is combined into a single state machine structure and moved
together from the stack into the heap_. This action is the only allocation
imposed by the futures library. In effect, the `Task` behaves as if you had
written an efficient state machine by hand while allowing you to express that
state machine as straight-line sequence of computations.

[`forget`]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#method.forget

Conceptually a [`Task`] is somewhat similar to an OS thread's stack. Where an OS
thread runs functions that all have access to the stack which is available
across blocking I/O calls, an asynchronous computation runs individual futures
over time which all have access to a [`Task`] that is persisted throughout the
lifetime of the computation.

---

## Task-local data
[task-local-data]: #task-local-data

[Back to top][top]

In the previous section we've now seen how each individual future is only
one piece of a larger asynchronous computation. This means that futures come
and go, but there could also be data that lives for the entire span of a
computation that many futures need access to.

Futures themselves require `Send + 'static`, so we have two choices to share
data between futures:

* If the data is only ever used by one future at a time we can thread through
  ownership of the data between each future.
* If the data needs to be accessed concurrently, however, then we'd have to
  naively store data in an `Arc` or worse, in an `Arc<Mutex>` if we wanted
  to mutate it.

But both of these solutions are relatively heavyweight, so let's see if we
can do better!

In the [`Task` and `Future`][task-and-future] section we saw how an asynchronous
computation has access to a [`Task`] for its entire lifetime, and from the
signature of [`poll`] and [`schedule`] we also see that it has mutable access to
this task. The [`Task`] API leverages these facts and allows you to store
data inside a `Task`.

Data is stored inside a `Task` with [`insert`] which returns a [`TaskData`]
handle. This handle can then be cloned regardless of the underlying data. To
access the data at a later time you can use the [`get`] or [`get_mut`] methods.

[`insert`]: http://alexcrichton.com/futures-rs/futures/struct.Task.html#method.insert
[`TaskData`]: http://alexcrichton.com/futures-rs/futures/struct.TaskData.html
[`get`]: http://alexcrichton.com/futures-rs/futures/struct.Task.html#method.get
[`get_mut`]: http://alexcrichton.com/futures-rs/futures/struct.Task.html#method.get_mut

A [`TaskData`] can also be created with the [`store`] future which will resolve
to a handle to the data being stored. Currently there is no combinator for
accessing data from a task and it's primarily used in manual implementations of
[`Future`], but this may change soon!

[`store`]: http://alexcrichton.com/futures-rs/futures/fn.store.html

---

## Event loop data
[event-loop-data]: #event-loop-data

[Back to top][top]

We've now seen that we can store data into a [`Task`] with [`TaskData`], but
this requires that the data inserted is still `Send`. Sometimes data is not
`Send` or otherwise needs to be persisted yet not tied to a [`Task`]. For this
purpose the [`futures-mio`] crate provides a similar abstraction, [`LoopData`].

[`LoopData`]: http://alexcrichton.com/futures-rs/futures_mio/struct.LoopData.html

The [`LoopData`] is similar to [`TaskData`] where it's a handle to data
conceptually owned by the event loop. The key property of [`LoopData`], however,
is that it implements the `Send` trait regardless of the underlying data.

A [`LoopData`] handle is a bit easier to access than a [`TaskData`], as you
don't need to get the data from a task. Instead you can simply attempt to access
the data with [`get`][loop-get] or [`get_mut`][loop-get-mut]. Both of these
methods return `Option` where `None` is returned if you're not on the event
loop.

[loop-get]: http://alexcrichton.com/futures-rs/futures_mio/struct.LoopData.html#method.get
[loop-get-mut]: http://alexcrichton.com/futures-rs/futures_mio/struct.LoopData.html#method.get_mut

In the case that `None` is returned, a future may have to return to the event
loop in order to make progress. In order to guarantee this the [`executor`]
associated with the data can be acquired and passed to [`Task::poll_on`]. This
will request that the task poll itself on the specified executor, which in this
case will run the poll request on the event loop where the data can be accessed.

[`executor`]: http://alexcrichton.com/futures-rs/futures_mio/struct.LoopData.html#method.executor
[`Task::poll_on`]: http://alexcrichton.com/futures-rs/futures/struct.Task.html#method.poll_on

A [`LoopData`] can be created through one of two methods:

* If you've got a handle to the [event loop itself][`Loop`] then you can call
  the [`Loop::add_loop_data`] method. This allows inserting data directly and a
  handle is immediately returned.
* If all you have is a [`LoopHandle`] then you can call the
  [`LoopHandle::add_loop_data`] method. This, unlike [`Loop::add_loop_data`],
  requires a `Send` closure which will be used to create the relevant data. Also
  unlike the [`Loop`] method, this function will return a future that resolves
  to the [`LoopData`] value.

[`Loop`]: http://alexcrichton.com/futures-rs/futures_mio/struct.Loop.html
[`Loop::add_loop_data`]: http://alexcrichton.com/futures-rs/futures_mio/struct.Loop.html#method.add_loop_data
[`LoopHandle`]: http://alexcrichton.com/futures-rs/futures_mio/struct.LoopHandle.html
[`LoopHandle::add_loop_data`]: http://alexcrichton.com/futures-rs/futures_mio/struct.LoopHandle.html#method.add_loop_data

Task-local data and event-loop data provide the ability for futures to easily
share sendable and non-sendable data amongst many futures.

