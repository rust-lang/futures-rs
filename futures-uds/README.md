# futures-uds

An implementation of an asynchronous HTTP client using futures backed by
libuds.

[![Build Status](https://travis-ci.org/alexcrichton/futures-rs.svg?branch=master)](https://travis-ci.org/alexcrichton/futures-rs)

[Documentation](http://alexcrichton.com/futures-rs/futures_uds)

> **Note**: this library does not currently work on Windows, but support is
> planned soon!

## Usage

First, add this to your `Cargo.toml`:

```toml
[dependencies]
futures-uds = { git = "https://github.com/alexcrichton/futures-rs" }
```

Next, add this to your crate:

```rust
extern crate futures_uds;
```

# License

`futures-uds` is primarily distributed under the terms of both the MIT
license and the Apache License (Version 2.0), with portions covered by various
BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.

