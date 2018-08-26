---
layout: post
title:  "Compatibility Layer"
subtitle: "0.1 ❤ 0.3"
author: "Josef Brandl"
author_github: "MajorBreakfast"
date:   2018-08-15
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
- TryFutureExt::compat: needs to be TryFuture => .unit_error(), needs to be Unpin => .boxed()
- TokioDefaultSpawner and briefly mention Executor01CompatExt::compat

## 0.1 futures in async functions
- `Future01CompatExt::compat`
