#![no_std]

//! An internal helper crate to workaround limitations in the
//! `use_extern_macros` feature with re-exported Macros 1.0 macros.
//!
//! All macros defined here should be imported via [`futures::prelude`] instead.

/// Await a sub-future inside an `#[async]` function.
///
/// You should pass an object implementing [`Future`] or [`StableFuture`] to
/// this macro, it will implicitly `yield` while that future returns
/// [`Async::Pending`] and evaluate to a [`Result`] containing the result of
/// that future once complete.
///
/// # Examples
///
#[cfg_attr(feature = "nightly", doc = "```")]
#[cfg_attr(not(feature = "nightly"), doc = "```ignore")]
/// #![feature(proc_macro, generators, pin)]
/// extern crate futures;
///
/// use futures::prelude::*;
/// use futures::future;
/// use futures::stable::block_on_stable;
///
/// #[async]
/// fn probably_one() -> Result<u32, u32> {
///     let one = await!(future::ok::<u32, u32>(1))?;
///     Ok(one)
/// }
///
/// assert_eq!(Ok(1), block_on_stable(probably_one()));
/// ```

#[macro_export]
macro_rules! await {
    ($e:expr) => ({
        // This macro is just here for documetation purposes, see
        // futures-macro-async/src/lib.rs for the implementation.
        __await!($e)
    })
}

/// Await an item from a stream inside an `#[async]` function.
///
/// You should pass an object implementing [`Stream`] to this macro, it will
/// implicitly `yield` while that stream returns [`Async::Pending`] and evaluate
/// to a [`Result`] containing the next item or error from that stream.
///
/// If you want to iterate over all items in a `Stream` you should instead see
/// the documentation on `#[async] for` in the main `#[async]` documentation.
///
/// # Examples
///
#[cfg_attr(feature = "nightly", doc = "```")]
#[cfg_attr(not(feature = "nightly"), doc = "```ignore")]
/// #![feature(proc_macro, generators, pin)]
/// extern crate futures;
///
/// use futures::prelude::*;
/// use futures::stream;
/// use futures::stable::block_on_stable;
///
/// #[async]
/// fn eventually_ten() -> Result<u32, u32> {
///     let mut stream = stream::repeat::<u32, u32>(5);
///     if let Some(first) = await_item!(stream)? {
///         if let Some(second) = await_item!(stream)? {
///             return Ok(first + second);
///         }
///     }
///     Err(0)
/// }
///
/// assert_eq!(Ok(10), block_on_stable(eventually_ten()));
/// ```

#[macro_export]
macro_rules! await_item {
    ($e:expr) => ({
        loop {
            let poll = ::futures::__rt::in_ctx(|ctx| ::futures::Stream::poll_next(&mut $e, ctx));
            // Allow for #[feature(never_type)] and Stream<Error = !>
            #[allow(unreachable_code, unreachable_patterns)]
            match poll {
                ::futures::__rt::std::result::Result::Ok(::futures::__rt::Async::Ready(e)) => {
                    break ::futures::__rt::std::result::Result::Ok(e)
                }
                ::futures::__rt::std::result::Result::Ok(::futures::__rt::Async::Pending) => {}
                ::futures::__rt::std::result::Result::Err(e) => {
                    break ::futures::__rt::std::result::Result::Err(e)
                }
            }

            yield ::futures::__rt::Async::Pending
        }
    })
}

/// Yield an item from an `#[async_stream]` function.
///
/// # Examples
///
#[cfg_attr(feature = "nightly", doc = "```")]
#[cfg_attr(not(feature = "nightly"), doc = "```ignore")]
/// #![feature(proc_macro, generators, pin)]
/// extern crate futures;
///
/// use futures::prelude::*;
/// use futures::stream;
/// use futures::executor::block_on;
///
/// #[async_stream_move(item = u32)]
/// fn one_five() -> Result<(), u32> {
///     stream_yield!(5);
///     Ok(())
/// }
///
/// assert_eq!(Ok(vec![5]), block_on(one_five().collect()));
/// ```

// TODO: This macro needs to use an extra temporary variable because of
// rust-lang/rust#44197, once that's fixed this should just use $e directly
// inside the yield expression
#[macro_export]
macro_rules! stream_yield {
    ($e:expr) => ({
        let e = $e;
        yield ::futures::__rt::Async::Ready(e)
    })
}
