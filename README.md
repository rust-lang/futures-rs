<p align="center">
  <img alt="futures-rs" src="https://raw.githubusercontent.com/rust-lang/futures-rs/gh-pages/assets/images/futures-rs-logo.svg?sanitize=true" width="400">
</p>

<p align="center">
  Zero-cost asynchronous programming in Rust
</p>

<p align="center">
  <a href="https://img.shields.io/github/workflow/status/rust-lang/futures-rs/CI/master">
    <img alt="Build Status" src="https://github.com/rust-lang/futures-rs/actions?query=branch%3Amaster">
  </a>

  <a href="https://crates.io/crates/futures">
    <img alt="Crates.io" src="https://img.shields.io/crates/v/futures.svg">
  </a>

  <a href="https://blog.rust-lang.org/2019/11/07/Rust-1.39.0.html">
    <img alt="Rustc Version" src="https://img.shields.io/badge/rustc-1.39+-lightgray.svg">
  </a>
</p>

<p align="center">
  <a href="https://docs.rs/futures/">
    Documentation
  </a> | <a href="https://rust-lang.github.io/futures-rs/">
    Website
  </a>
</p>

`futures-rs` is a library providing the foundations for asynchronous programming in Rust.
It includes key trait definitions like `Stream`, as well as utilities like `join!`,
`select!`, and various futures combinator methods which enable expressive asynchronous
control flow.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
futures = "0.3"
```

Now, you can use futures-rs:

```rust
use futures::future::Future;
```

The current futures-rs requires Rust 1.39 or later.

### Feature `std`

Futures-rs works without the standard library, such as in bare metal environments.
However, it has a significantly reduced API surface. To use futures-rs in
a `#[no_std]` environment, use:

```toml
[dependencies]
futures = { version = "0.3", default-features = false }
```

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in futures-rs by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
