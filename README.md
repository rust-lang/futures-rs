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
    <img alt="Crates.io" src="https://img.shields.io/crates/v/futures-preview.svg">
  </a>
</p>

<p align="center">
  <a href="https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.11/futures/">
    Documentation
  </a> | <a href="https://rust-lang-nursery.github.io/futures-rs/">
    Website
  </a>
</p>

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
futures-preview = "0.3.0-alpha.11"
```

Now, you can use futures-rs:

```rust
use futures::future::Future; // Note: It's not `futures_preview`
```

The current version of futures-rs requires Rust nightly 2019-01-11 or later.

### Feature `std`

Futures-rs works without the standard library, such as in bare metal environments.
However, it has a significantly reduced API surface. To use futures-rs in
a `#[no_std]` environment, use:

```toml
[dependencies]
futures-preview = { version = "0.3.0-alpha.11", default-features = false }
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
for inclusion in futures-rs by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
