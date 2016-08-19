# FAQ

A collection of some commonly asked questions, with responses! If you find any
of these unsatisfactory feel free to ping me (@alexcrichton) on github,
acrichto on IRC, or just by email!

### Why both `Item` and `Error` associated types?

An alternative design of the `Future` trait would be to only have one associated
type, `Item`, and then most futures would resolve to `Result<T, E>`. The
intention of futures, the fundamental support for async I/O, typically means
that errors will be encoded in almost all futures anyway though. By encoding an
error type in the future as well we're able to provide convenient combinators
like `and_then` which automatically propagate errors, as well as combinators
like `join` which can act differently depending on whether a future resolves to
an error or not.

### Do futures work with multiple event loops?

Yes! Futures are designed to source events from any location, including multiple
event loops. All of the basic combinators will work on any number of event loops
across any number of threads.

### What if I have CPU intensive work?

The documentation of the `Future::poll` function says that's it's supposed to
"return quickly", what if I have work that doesn't return quickly! In this case
it's intended that this work will run on a dedicated pool of threads intended
for this sort of work, and a future to the returned value is used to represent
its completion.

A proof-of-concept method of doing this is the `futures-cpupool` crate in this
repository, where you can execute work on a thread pool and receive a future to
the value generated. This future is then composable with `and_then`, for
example, to mesh in with the rest of a future's computation.

### How do I call `poll`?

Right now, call `.forget()`. That method will drive a future to completion and
drop all associated resources as soon as it's completed.

Eventually more flavorful methods of configuring a `Task` will be available, but
right now `.forget()` is all we have.

### How do I return a future?

Returning a future is like returning an iterator in Rust today. It's not the
easiest thing to do and you frequently need to resort to `Box` with a trait
object. Thankfully though [`impl Trait`] is just around the corner and will
allow returning these types unboxed in the future.

[`impl Trait`]: https://github.com/rust-lang/rust/issues/34511

For now though the cost of boxing shouldn't actually be that high. A future
computation can be constructed *without boxing* and only the final step actually
places a `Box` around the entire future. In that sense you're only paying the
allocation at the very end, not for any of the intermediate futures.

More information can be found [in the tutorial][return-future].

[return-future]: https://github.com/alexcrichton/futures-rs/blob/master/TUTORIAL.md#returning-futures

### Does it work on Windows?

Yes! This library builds on top of mio, which works on Windows.

### What version of Rust should I use?

Rust 1.10 or later.

### Is it on crates.io?

Not yet! A few names are reserved, but crates cannot have dependencies from a
git repository. Right now we depend on the master branch of `mio`, and crates
will be published once that's on crates.io as well!
