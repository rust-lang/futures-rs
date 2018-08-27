---
layout: post
title:  "Compatibility Layer"
subtitle: "0.1 ❤ 0.3"
author: "Josef Brandl"
author_github: "MajorBreakfast"
date:   2018-01-01
categories: blog
---

# Futures 0.1 Compatibility Layer

Rust's futures ecosystem is currenlty split in two: On the one hand we have the vibrant ecosystem built around `futures@0.1` with its many libraries working on stable Rust and on the other hand there's the unstable `futures@0.3` ecosystem with support for the ergonomic and powerful `async`/`await` language feature. To bridge the gap between these two worlds we've introduced with `0.3.0-alpha.3` a compatibility layer that makes it possible to use `futures@0.1` and `futures@0.3` futures and streams together in one project. This blog post aims to give an overview over how to use it.

## `Cargo.toml`

The compatibility layer can be enabled by setting the `compat` feature your `Cargo.toml`:

```toml
futures-preview = { version = "0.3.0-alpha.3", features = ["compat"] }
```

To use `futures@0.1` and `futures@0.3` together in a single project, we can make use of the new cargo feature for renaming dependencies. Why? Because, even though the `futures@0.3` crate is called `futures-preview` on crates.io, it's lib name is also `futures`. By renaming `futures` version 0.1 to `futures01`, we can avoid a name colision:

```toml
# A the top:
cargo-features = ["rename-dependency"]

[dependencies]
futures01 = { package = "futures", version = "0.1", optional = true }
```

**Note: Renaming the crate is only required if you specify it as a dependency. If your project depends on Tokio and thus only indirectly on `futures@0.1`, then no renaming is required.**

## Async functions on 0.1 executors

The compatibility layer makes it possible to run 0.3 futures on executors built for 0.1. This makes it for instance possible to run futures created via `async`/`await` on Tokio's executor. Here's how this looks like:

```rust
#![feature(async_await, await_macro, futures_api)]
use futures::future::{FutureExt, TryFutureExt};
use futures::compat::TokioDefaultSpawner;

let future03 = async {
    println!("Running on the pool");
};

let future01 = future03
    .unit_error()
    .boxed()
    .compat(TokioDefaultSpawner);

tokio::run(future01);
```

Turning a 0.3 future into a 0.1 future requires three steps:
- First, the future needs to be a `TryFuture`, i.e. a future with `Output = Result<T, E>`. If your future isn't a `TryFuture` yet, you can quickly make it one using the `unit_error` combinator which wraps the output in a `Result<T, ()>`.
- Next, the future needs to be `Unpin`. If your future isn't `Unpin` yet, you can use the `boxed` combinator which wraps the future in a `PinBox`.
- The final step is to call the `compat` combinator which converts it into a future that can run both on 0.1 and 0.3 executors. This method requires a `spawner` parameter because 0.1 futures don't get passed a context that contains a spawner. If you use Tokio's default executor, you can do it like in the example above. Otherwise, take a look at the code example for `Executor01CompatExt::compat` if you want to specify a custom spawner.

## 0.1 futures in async functions

The conversion from a 0.1 future to a 0.3 future also works via a `compat` combinator method:

```rust
use futures::compat::Futures01CompatExt;

let future03 = future01.compat();
```

It converts a 0.1 `Future<Item = T, Error = E>` into a 0.3 `Future<Output = Result<T, E>>`.

## Streams

Converting between 0.1 and 0.3 streams is possible via the `TryStreamExt::compat` and `Stream01CompatExt::compat` methods. Both combinators work analogously to their future equivalents.

## Conclusion

The compatiblity layer offers conversions in both directions and thus enables gradual migrations and experiments with futures 0.3. With that it manages to bridge the gap between the futures 0.1 and futures 0.3 ecosystems.

Finally a self contained example that shows how to fetch a website from a server:

```rust
#![feature(pin, async_await, await_macro)]
use futures::compat::{Future01CompatExt, Stream01CompatExt, TokioDefaultSpawner};
use futures::stream::{StreamExt};
use futures::future::{TryFutureExt, FutureExt};
use hyper::Client;
use pin_utils::pin_mut;
use std::io::{self, Write};

fn main() {
    let future03 = async {
        let url = "http://httpbin.org/ip".parse().unwrap();

        let client = Client::new();
        let res = await!(client.get(url).compat()).unwrap();
        println!("{}", res.status());

        let body = res.into_body().compat();
        pin_mut!(body);
        while let Some(Ok(chunk)) = await!(body.next()) {
            io::stdout()
                .write_all(&chunk)
                .expect("example expects stdout is open");
        }
    };

    tokio::run(future03.unit_error().boxed().compat(TokioDefaultSpawner))
}
```

Special thanks goes to [@tinaun](https://www.github.com/tinaun) and [@Nemo157](https://www.github.com/Nemo157) for developing the compatibility layer.
