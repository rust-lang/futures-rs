<p align="center">
  <img alt="futures-rs" src="https://raw.githubusercontent.com/rust-lang-nursery/futures-rs/gh-pages/assets/images/futures-rs-logo.svg?sanitize=true" width="400">
</p>

<p align="center">
  Zero-cost asynchronous programming in Rust
</p>

<p align="center">
  <a href="https://travis-ci.com/rust-lang-nursery/futures-rs">
    <img alt="Build Status" src="https://travis-ci.com/rust-lang-nursery/futures-rs.svg?branch=master">
  </a>

  <a href="https://crates.io/crates/futures-preview">
    <img alt="Crates.io" src="https://img.shields.io/crates/v/futures-preview.svg">
  </a>

  <a href="https://blog.rust-lang.org/2019/07/04/Rust-1.36.0.html">
    <img alt="Rustc Version" src="https://img.shields.io/badge/rustc-1.36+-lightgray.svg">
  </a>
</p>

<p align="center">
  <a href="https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.18/futures/">
    Documentation
  </a> | <a href="https://rust-lang-nursery.github.io/futures-rs/">
    Website
  </a>
</p>

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
futures-preview = "=0.3.0-alpha.18"
```

Now, you can use futures-rs:

```rust
use futures::future::Future; // Note: It's not `futures_preview`
```

The current futures-rs requires Rust 1.36 or later.

### Feature `std`

Futures-rs works without the standard library, such as in bare metal environments.
However, it has a significantly reduced API surface. To use futures-rs in
a `#[no_std]` environment, use:

```toml
[dependencies]
futures-preview = { version = "=0.3.0-alpha.18", default-features = false }
```

### Feature `async-await`

The `async-await` feature provides several convenient features using unstable
async/await. Note that this is an unstable feature, and upstream changes might
prevent it from compiling. To use futures-rs with async/await, use:

```toml
[dependencies]
futures-preview = { version = "=0.3.0-alpha.18", features = ["async-await", "nightly"] }
```

The current `async-await` feature requires Rust nightly 2019-07-29 or later.

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
