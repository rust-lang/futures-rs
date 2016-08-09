# futures-curl

An implementation of an asynchronous HTTP client using futures backed by
libcurl.

[![Build Status](https://travis-ci.org/alexcrichton/futures-rs.svg?branch=master)](https://travis-ci.org/alexcrichton/futures-rs)

[Documentation](http://alexcrichton.com/futures-rs/futures_curl)

## Usage

First, add this to your `Cargo.toml`:

```toml
[dependencies]
futures-curl = { git = "https://github.com/alexcrichton/futures-rs" }
```

Next, add this to your crate:

```rust
extern crate futures_curl;
```

# License

`futures-curl` is primarily distributed under the terms of both the MIT
license and the Apache License (Version 2.0), with portions covered by various
BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
