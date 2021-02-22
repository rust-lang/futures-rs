#![cfg_attr(feature = "fntraits", doc = r#"
Wrap `Fn`, `FnMut`, and `FnOnce` traits allowing for more concrete typed adapters.
# This module contains
## Fn-trait wrappers
- [`FnOnce0`], [`FnMut0`], and [`Fn0`] traits mimicking `Fn*() -> Output`.
- [`FnOnce1`], [`FnMut1`], and [`Fn1`] traits mimicking `Fn*(A) -> Output`.
- [`FnOnce2`], [`FnMut2`], and [`Fn2`] traits mimicking `Fn*(A, B) -> Output`.
## Dual adapters from other top-level modules
The dual API using the above fn-traits is accessible in via [`future`], [`sink`], and [`stream`]
module, analogously to the original version.
Using the trait wrappers, for all combinators and utility functions taking closures into type
parameters an adapted version is provided.
This enables explicit typed versions when implementing corresponding fn-traits.

# Example

```
use futures_util::fns::{FnOnce1, FnMut1, Fn1, stream::StreamExtFns};
use futures_util::stream::{self, Stream, StreamExt};
struct Adder(i32);

impl FnOnce1<i32> for Adder {
    type Output = i32;
    fn call_once(self, val: i32) -> Self::Output {
        self.0 + val
    }
}
impl FnMut1<i32> for Adder {
    fn call_mut(&mut self, val: i32) -> Self::Output {
        self.0 + val
    }
}

impl Fn1<i32> for Adder {
    fn call(&self, val: i32) -> Self::Output {
        self.0 + val
    }
}

fn shift_stream<St>(stream: St, shift: i32) -> stream::Map<St, Adder>
where
    St: Stream<Item = i32>,
{
    StreamExtFns::map(stream, Adder(shift))
}

futures::executor::block_on(async {
    let stream = shift_stream(stream::iter(0..4), -42);
    assert_eq!(vec![-42, -41, -40, -39], stream.collect::<Vec<_>>().await);
})
```
"#)]
#![allow(missing_docs)]

use core::fmt::{self, Debug};
use core::marker::PhantomData;

//--------------------------------------------------------------------
// NO ARGUMENT
//--------------------------------------------------------------------

/// Like [`FnOnce`] taking no argument but implementable.
pub trait FnOnce0 {
    /// The returned type after the call operator is used.
    type Output;
    /// Performs the call operation.
    fn call_once(self) -> Self::Output;
}

impl<F, R> FnOnce0 for F
where
    F: FnOnce() -> R,
{
    type Output = R;
    #[inline]
    fn call_once(self) -> R {
        self()
    }
}

/// Like [`FnMut`] taking no argument but implementable.
pub trait FnMut0: FnOnce0 {
    /// Performs the call operation.
    fn call_mut(&mut self) -> Self::Output;
}

impl<F, R> FnMut0 for F
where
    F: FnMut() -> R,
{
    #[inline]
    fn call_mut(&mut self) -> R {
        self()
    }
}

// Not used, but present for completeness
#[allow(unreachable_pub)]
#[doc(hide)]
/// Like [`Fn`] taking no argument but implementable.
pub trait Fn0: FnMut0 {
    /// Performs the call operation.
    fn call(&self) -> Self::Output;
}

impl<F, R> Fn0 for F
where
    F: Fn() -> R,
{
    #[inline]
    fn call(&self) -> R {
        self()
    }
}

//--------------------------------------------------------------------
// SINGLE ARGUMENT
//--------------------------------------------------------------------

/// Like [`FnOnce`] taking exactly one argument but implementable.
pub trait FnOnce1<A> {
    /// The returned type after the call operator is used.
    type Output;
    /// Performs the call operation.
    fn call_once(self, arg: A) -> Self::Output;
}

impl<T, A, R> FnOnce1<A> for T
where
    T: FnOnce(A) -> R
{
    type Output = R;
    #[inline]
    fn call_once(self, arg: A) -> R {
        self(arg)
    }
}

/// Like [`FnMut`] taking exactly one argument but implementable.
pub trait FnMut1<A>: FnOnce1<A> {
    /// Performs the call operation.
    fn call_mut(&mut self, arg: A) -> Self::Output;
}

impl<T, A, R> FnMut1<A> for T
where
    T: FnMut(A) -> R
{
    #[inline]
    fn call_mut(&mut self, arg: A) -> R {
        self(arg)
    }
}

// Not used, but present for completeness
#[allow(unreachable_pub)]
#[doc(hide)]
/// Like [`Fn`] taking exactlyexactlyexactlyexactly one argument but implementable.
pub trait Fn1<A>: FnMut1<A> {
    /// Performs the call operation.
    fn call(&self, arg: A) -> Self::Output;
}

impl<T, A, R> Fn1<A> for T
where
    T: Fn(A) -> R
{
    #[inline]
    fn call(&self, arg: A) -> R {
        self(arg)
    }
}

macro_rules! trivial_fn_impls {
    ($name:ident <$($arg:ident),*> $t:ty = $debug:literal) => {
        impl<$($arg),*> Copy for $t {}
        impl<$($arg),*> Clone for $t {
            fn clone(&self) -> Self { *self }
        }
        impl<$($arg),*> Debug for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str($debug)
            }
        }
        impl<$($arg,)* A> FnMut1<A> for $t where Self: FnOnce1<A> {
            fn call_mut(&mut self, arg: A) -> Self::Output {
                self.call_once(arg)
            }
        }
        impl<$($arg,)* A> Fn1<A> for $t where Self: FnOnce1<A> {
            fn call(&self, arg: A) -> Self::Output {
                self.call_once(arg)
            }
        }
        pub(crate) fn $name<$($arg),*>() -> $t {
            Default::default()
        }
    }
}

#[doc(hidden)]
pub struct OkFn<E>(PhantomData<fn(E)>);

impl<E> Default for OkFn<E> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<A, E> FnOnce1<A> for OkFn<E> {
    type Output = Result<A, E>;
    fn call_once(self, arg: A) -> Self::Output {
        Ok(arg)
    }
}

trivial_fn_impls!(ok_fn <T> OkFn<T> = "Ok");

#[doc(hidden)]
#[derive(Debug, Copy, Clone, Default)]
pub struct ChainFn<F, G>(F, G);

impl<F, G, A> FnOnce1<A> for ChainFn<F, G>
where
    F: FnOnce1<A>,
    G: FnOnce1<F::Output>,
{
    type Output = G::Output;
    fn call_once(self, arg: A) -> Self::Output {
        self.1.call_once(self.0.call_once(arg))
    }
}
impl<F, G, A> FnMut1<A> for ChainFn<F, G>
where
    F: FnMut1<A>,
    G: FnMut1<F::Output>,
{
    fn call_mut(&mut self, arg: A) -> Self::Output {
        self.1.call_mut(self.0.call_mut(arg))
    }
}
impl<F, G, A> Fn1<A> for ChainFn<F, G>
where
    F: Fn1<A>,
    G: Fn1<F::Output>,
{
    fn call(&self, arg: A) -> Self::Output {
        self.1.call(self.0.call(arg))
    }
}
pub(crate) fn chain_fn<F, G>(f: F, g: G) -> ChainFn<F, G> {
    ChainFn(f, g)
}

#[doc(hidden)]
#[derive(Default)]
pub struct MergeResultFn;

impl<T> FnOnce1<Result<T, T>> for MergeResultFn {
    type Output = T;
    fn call_once(self, arg: Result<T, T>) -> Self::Output {
        match arg {
            Ok(x) => x,
            Err(x) => x,
        }
    }
}
trivial_fn_impls!(merge_result_fn <> MergeResultFn = "merge_result");

//--------------------------------------

// as the delgate macro fails with HRTBs use the following wrapper traits

#[doc(hidden)]
#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
pub trait FnOnceRef1<A>: for<'a> FnOnce1<&'a A, Output = ()> {}

#[doc(hidden)]
#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<A, F: for<'a> FnOnce1<&'a A, Output = ()>> FnOnceRef1<A> for F {}

#[doc(hidden)]
#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
pub trait FnMutRef1<A>: for<'a> FnMut1<&'a A, Output = ()> + FnOnceRef1<A> {}

#[doc(hidden)]
#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<A, F: for<'a> FnMut1<&'a A, Output = ()>> FnMutRef1<A> for F {}

// Not used, but present for completeness
#[allow(unreachable_pub)]
#[doc(hidden)]
#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
pub trait FnRef1<A>: for<'a> Fn1<&'a A, Output = ()> + FnMutRef1<A> {}

#[doc(hidden)]
#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<A, F: for<'a> Fn1<&'a A, Output = ()>> FnRef1<A> for F {}

mod futures_test {
    // needed by futures test at futures/tests/auto_traits.rs
    use super::{FnOnce1, FnMut1, Fn1};

    macro_rules! dummy_impl {
        ($($type:ty),*) => {
            $(
                impl<'a, A> FnOnce1<&'a A> for $type {
                    type Output = ();
                    fn call_once(self, _: &'a A) -> Self::Output {
                        ()
                    }
                }
                impl<'a, A> FnMut1<&'a A> for $type {
                    fn call_mut(&mut self, _: &'a A) -> Self::Output {
                        ()
                    }
                }
                impl<'a, A> Fn1<&'a A> for $type {
                    fn call(&self, _: &'a A) -> Self::Output {
                        ()
                    }
                }
            )*
        };
    }

    dummy_impl!{ (), *const (), core::marker::PhantomPinned }
}
//--------------------------------------
#[doc(hidden)]
#[derive(Debug, Copy, Clone, Default)]
pub struct InspectFn<F>(F);

#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<F, A> FnOnce1<A> for InspectFn<F>
where
    F: for<'a> FnOnce1<&'a A, Output=()>,
{
    type Output = A;
    fn call_once(self, arg: A) -> Self::Output {
        self.0.call_once(&arg);
        arg
    }
}
#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<F, A> FnMut1<A> for InspectFn<F>
where
    F: for<'a> FnMut1<&'a A, Output=()>,
{
    fn call_mut(&mut self, arg: A) -> Self::Output {
        self.0.call_mut(&arg);
        arg
    }
}
#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<F, A> Fn1<A> for InspectFn<F>
where
    F: for<'a> Fn1<&'a A, Output=()>,
{
    fn call(&self, arg: A) -> Self::Output {
        self.0.call(&arg);
        arg
    }
}
pub(crate) fn inspect_fn<F>(f: F) -> InspectFn<F> {
    InspectFn(f)
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone, Default)]
pub struct MapOkFn<F>(F);

impl<F, T, E> FnOnce1<Result<T, E>> for MapOkFn<F>
where
    F: FnOnce1<T>,
{
    type Output = Result<F::Output, E>;
    fn call_once(self, arg: Result<T, E>) -> Self::Output {
        arg.map(|x| self.0.call_once(x))
    }
}
impl<F, T, E> FnMut1<Result<T, E>> for MapOkFn<F>
where
    F: FnMut1<T>,
{
    fn call_mut(&mut self, arg: Result<T, E>) -> Self::Output {
        arg.map(|x| self.0.call_mut(x))
    }
}
impl<F, T, E> Fn1<Result<T, E>> for MapOkFn<F>
where
    F: Fn1<T>,
{
    fn call(&self, arg: Result<T, E>) -> Self::Output {
        arg.map(|x| self.0.call(x))
    }
}
pub(crate) fn map_ok_fn<F>(f: F) -> MapOkFn<F> {
    MapOkFn(f)
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone, Default)]
pub struct MapErrFn<F>(F);

impl<F, T, E> FnOnce1<Result<T, E>> for MapErrFn<F>
where
    F: FnOnce1<E>,
{
    type Output = Result<T, F::Output>;
    fn call_once(self, arg: Result<T, E>) -> Self::Output {
        arg.map_err(|x| self.0.call_once(x))
    }
}
impl<F, T, E> FnMut1<Result<T, E>> for MapErrFn<F>
where
    F: FnMut1<E>,
{
    fn call_mut(&mut self, arg: Result<T, E>) -> Self::Output {
        arg.map_err(|x| self.0.call_mut(x))
    }
}
impl<F, T, E> Fn1<Result<T, E>> for MapErrFn<F>
where
    F: Fn1<E>,
{
    fn call(&self, arg: Result<T, E>) -> Self::Output {
        arg.map_err(|x| self.0.call(x))
    }
}
pub(crate) fn map_err_fn<F>(f: F) -> MapErrFn<F> {
    MapErrFn(f)
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone)]
pub struct InspectOkFn<F>(F);

impl<'a, F, T, E> FnOnce1<&'a Result<T, E>> for InspectOkFn<F>
where
    F: FnOnce1<&'a T, Output=()>
{
    type Output = ();
    fn call_once(self, arg: &'a Result<T, E>) -> Self::Output {
        if let Ok(x) = arg { self.0.call_once(x) }
    }
}
impl<'a, F, T, E> FnMut1<&'a Result<T, E>> for InspectOkFn<F>
where
    F: FnMut1<&'a T, Output=()>,
{
    fn call_mut(&mut self, arg: &'a Result<T, E>) -> Self::Output {
        if let Ok(x) = arg { self.0.call_mut(x) }
    }
}
impl<'a, F, T, E> Fn1<&'a Result<T, E>> for InspectOkFn<F>
where
    F: Fn1<&'a T, Output=()>,
{
    fn call(&self, arg: &'a Result<T, E>) -> Self::Output {
        if let Ok(x) = arg { self.0.call(x) }
    }
}
pub(crate) fn inspect_ok_fn<F>(f: F) -> InspectOkFn<F> {
    InspectOkFn(f)
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone)]
pub struct InspectErrFn<F>(F);

impl<'a, F, T, E> FnOnce1<&'a Result<T, E>> for InspectErrFn<F>
where
    F: FnOnce1<&'a E, Output=()>
{
    type Output = ();
    fn call_once(self, arg: &'a Result<T, E>) -> Self::Output {
        if let Err(x) = arg { self.0.call_once(x) }
    }
}
impl<'a, F, T, E> FnMut1<&'a Result<T, E>> for InspectErrFn<F>
where
    F: FnMut1<&'a E, Output=()>,
{
    fn call_mut(&mut self, arg: &'a Result<T, E>) -> Self::Output {
        if let Err(x) = arg { self.0.call_mut(x) }
    }
}
impl<'a, F, T, E> Fn1<&'a Result<T, E>> for InspectErrFn<F>
where
    F: Fn1<&'a E, Output=()>,
{
    fn call(&self, arg: &'a Result<T, E>) -> Self::Output {
        if let Err(x) = arg { self.0.call(x) }
    }
}
pub(crate) fn inspect_err_fn<F>(f: F) -> InspectErrFn<F> {
    InspectErrFn(f)
}

pub(crate) type MapOkOrElseFn<F, G> = ChainFn<MapOkFn<F>, ChainFn<MapErrFn<G>, MergeResultFn>>;
pub(crate) fn map_ok_or_else_fn<F, G>(f: F, g: G) -> MapOkOrElseFn<F, G> {
    chain_fn(map_ok_fn(f), chain_fn(map_err_fn(g), merge_result_fn()))
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone, Default)]
pub struct UnwrapOrElseFn<F>(F);

impl<F, T, E> FnOnce1<Result<T, E>> for UnwrapOrElseFn<F>
where
    F: FnOnce1<E, Output=T>,
{
    type Output = T;
    fn call_once(self, arg: Result<T, E>) -> Self::Output {
        arg.unwrap_or_else(|x| self.0.call_once(x))
    }
}
impl<F, T, E> FnMut1<Result<T, E>> for UnwrapOrElseFn<F>
where
    F: FnMut1<E, Output=T>,
{
    fn call_mut(&mut self, arg: Result<T, E>) -> Self::Output {
        arg.unwrap_or_else(|x| self.0.call_mut(x))
    }
}
impl<F, T, E> Fn1<Result<T, E>> for UnwrapOrElseFn<F>
where
    F: Fn1<E, Output=T>,
{
    fn call(&self, arg: Result<T, E>) -> Self::Output {
        arg.unwrap_or_else(|x| self.0.call(x))
    }
}
pub(crate) fn unwrap_or_else_fn<F>(f: F) -> UnwrapOrElseFn<F> {
    UnwrapOrElseFn(f)
}

#[doc(hidden)]
pub struct IntoFn<T>(PhantomData<fn() -> T>);

impl<T> Default for IntoFn<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}
impl<A, T> FnOnce1<A> for IntoFn<T> where A: Into<T> {
    type Output = T;
    fn call_once(self, arg: A) -> Self::Output {
        arg.into()
    }
}

trivial_fn_impls!(into_fn <T> IntoFn<T> = "Into::into");
//--------------------------------------------------------------------
// TWO ARGUMENTS
//--------------------------------------------------------------------

/// Like [`FnOnce`] taking exactly two arguments but implementable.
pub trait FnOnce2<A, B> {
    /// The returned type after the call operator is used.
    type Output;
    /// Performs the call operation.
    fn call_once(self, arg1: A, arg2: B) -> Self::Output;
}

impl<F, A, B, R> FnOnce2<A, B> for F
where
    F: FnOnce(A, B) -> R,
{
    type Output = R;
    #[inline]
    fn call_once(self, arg1: A, arg2: B) -> R {
        self(arg1, arg2)
    }
}

/// Like [`FnMut`] taking exactly two arguments but implementable.
pub trait FnMut2<A, B>: FnOnce2<A, B> {
    /// Performs the call operation.
    fn call_mut(&mut self, arg1: A, arg2: B) -> Self::Output;
}

impl<F, A, B, R> FnMut2<A, B> for F
where
    F: FnMut(A, B) -> R,
{
    #[inline]
    fn call_mut(&mut self, arg1: A, arg2: B) -> R {
        self(arg1, arg2)
    }
}

// Not used, but present for completeness
#[allow(unreachable_pub)]
#[doc(hide)]
/// Like [`Fn`] taking no argument but implementable.
pub trait Fn2<A, B>: FnMut2<A, B> {
    /// Performs the call operation.
    fn call(&self, arg1: A, arg2: B) -> Self::Output;
}

impl<F, A, B, R> Fn2<A, B> for F
where
    F: Fn(A, B) -> R,
{
    #[inline]
    fn call(&self, arg1: A, arg2: B) -> R {
        self(arg1, arg2)
    }
}

//--------------------------------------------------------------------
// provide uncluttered access to dual API
//
// note: for readability these are hidden from doc at there source modules

#[cfg(feature = "fntraits")]
pub mod future {
    //! Adaptations for [`future`](crate::future) module using fn-trait wrappers.
    #[doc(inline)]
    pub use crate::future::{
        FutureExtFns,
        TryFutureExtFns,
        lazy_fns as lazy,
        poll_fn_fns as poll_fs,
    };
}
#[cfg(all(feature = "fntraits", feature = "sink"))]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
pub mod sink {
    //! Adaptations for [`sink`](crate::sink) module using fn-trait wrappers.
    #[doc(inline)]
    pub use crate::sink::{
        SinkExtFns as SinkExt,
        unfold_fns as unfold,
    };
}
#[cfg(feature = "fntraits")]
pub mod stream {
    //! Adaptations for [`stream`](crate::stream) module using fn-trait wrappers.
    #[doc(inline)]
    pub use crate::stream::{
        StreamExtFns,
        TryStreamExtFns,
        poll_fn_fns as poll_fs,
        repeat_with_fns as repeat_with,
        try_unfold_fns as try_unfold,
        unfold_fns as unfold,
    };
}
