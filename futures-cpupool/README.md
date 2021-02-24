# futures-cpupool

A library for creating futures representing work happening concurrently on a
dedicated thread pool.

[![Build Status](https://img.shields.io/github/workflow/status/rust-lang/futures-rs/CI/master)](https://github.com/rust-lang/futures-rs/actions)

[Documentation](https://docs.rs/futures-cpupool)

## Usage

First, add this to your `Cargo.toml`:

```toml
[dependencies]
futures = "0.1"
futures-cpupool = "0.1"
```

Next, add this to your crate:

```rust
extern crate futures;
extern crate futures_cpupool;

use futures_cpupool::CpuPool;
```

# License

`futures-cpupool` is primarily distributed under the terms of both the MIT
license and the Apache License (Version 2.0), with portions covered by various
BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
