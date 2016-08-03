# FAQ

A collection of some commonly asked questions, with responses! If you find any
of these unsatisfactory feel free to ping me (@alexcrichton) on github,c
acrichto on IRC, or just by email!

### Why `Send + 'static`?

The `Future` trait and all of its associated items require `Send + 'static`.
This expressed the constraint that all futures must be sendable across threads
as well as not contain any borrowed data. A common question though is why not
just let this fall out of the types themselves? That is, I'll have `Send +
'static` futures if they happend to contain `Send + 'static` data.

On a technical level this is not currently possible. Due to the `tailcall`
method which flattens a chain of futures, futures commonly may store trait
objects. As trait objects must decide on `Send` and `'static` early on, we opted
to say "yes, futures will be both" early on.

Doesn't this impose an extra cost though? Other libraries only require `'static`
which allows one to use types like `Rc` and `RefCell` liberally. This is true
that futures themselves cannot contain data like an `Rc`, but it is planned that
through an *executor* you will be able to access non-`Send` data. This is not
currently implemented, but is coming soon!

The final reason is that almost all futures end up being `Send + 'static` in
practice. This allows for a convenient implementation of driving futures by
simply polling a future on whichever thread originates an event, ensuring a
prompt resolution of a future if one is available. This, when combined with the
technical difficulties and ergonomic concerns of *not* having `Send` and
`'static`, led to the conclusion that the trait will require both.

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

### How do I call `poll` and `schedule`?

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

### Does it work on Windows?

Yes! This library builds on top of mio, which works on Windows.

### What version of Rust should I use?

While the library compiles on stable and beta (as of 2016-08-02), the nightly
release (1.13.0-nightly) is recommended due to Cargo workspaces and compiler bug
fixes that make compilation much speedier.

### Is it on crates.io?

Not yet! A few names are reserved, but they're not functional. I'd use the git
repository here for now.





















