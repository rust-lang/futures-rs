<p align="center">
  <img alt="futures-rs" src="https://raw.githubusercontent.com/rust-lang-nursery/futures-rs/gh-pages/assets/images/futures-rs-logo.svg?sanitize=true" width="400">
</p>

<p align="center">
  Zero-cost asynchronous programming in Rust
</p>

<p align="center">
  <a href="https://travis-ci.org/rust-lang-nursery/futures-rs">
    <img alt="Build Status" src="https://travis-ci.org/rust-lang-nursery/futures-rs.svg?branch=master">
  </a>

  <a href="https://crates.io/crates/futures-preview">
    <img alt="Crates.io" src="https://img.shields.io/crates/v/futures-preview.svg?maxAge=2592000">
  </a>
</p>

<p align="center">
  <a href="https://rust-lang-nursery.github.io/futures-rs/doc/futures">
    Documentation
  </a> | <a href="https://rust-lang-nursery.github.io/futures-rs/">
    Website
  </a>
</p>

## Usage

First, add this to your `Cargo.toml`:

```toml
[dependencies]
futures-preview = "0.3.0-alpha.1"
```

Next, add this to your crate:

```rust
extern crate futures; // Note: It's not `futures_preview`

use futures::future::Future;
```

### Feature `std`

`futures-rs` works without the standard library, such as in bare metal environments.
However, it has a significantly reduced API surface. To use `futures-rs` in
a `#[no_std]` environment, use:

```toml
[dependencies]
futures-preview = { version = "0.3.0-alpha.1", default-features = false }
```

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Futures by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
