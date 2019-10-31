//! The futures-rs `join! macro implementation.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{parenthesized, parse_quote, Expr, Ident, Token};

mod kw {
    syn::custom_keyword!(futures_crate_path);
}

#[derive(Default)]
struct Join {
    futures_crate_path: Option<syn::Path>,
    fut_exprs: Vec<Expr>,
}

impl Parse for Join {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut join = Join::default();

        // When `futures_crate_path(::path::to::futures::lib)` is provided,
        // it sets the path through which futures library functions will be
        // accessed.
        if input.peek(kw::futures_crate_path) {
            input.parse::<kw::futures_crate_path>()?;
            let content;
            parenthesized!(content in input);
            join.futures_crate_path = Some(content.parse()?);
        }

        while !input.is_empty() {
            join.fut_exprs.push(input.parse::<Expr>()?);

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(join)
    }
}

fn bind_futures(
    futures_crate: &syn::Path,
    fut_exprs: Vec<Expr>,
    span: Span,
) -> (Vec<TokenStream2>, Vec<Ident>) {
    let mut future_let_bindings = Vec::with_capacity(fut_exprs.len());
    let future_names: Vec<_> = fut_exprs
        .into_iter()
        .enumerate()
        .map(|(i, expr)| {
            let name = format_ident!("_fut{}", i, span = span);
            future_let_bindings.push(quote! {
                // Move future into a local so that it is pinned in one place and
                // is no longer accessible by the end user.
                let mut #name = #futures_crate::future::maybe_done(#expr);
            });
            name
        })
        .collect();

    (future_let_bindings, future_names)
}

/// The `join!` macro.
pub(crate) fn join(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as Join);

    let futures_crate = parsed
        .futures_crate_path
        .unwrap_or_else(|| parse_quote!(::futures_util));

    // should be def_site, but that's unstable
    let span = Span::call_site();

    let (future_let_bindings, future_names) = bind_futures(&futures_crate, parsed.fut_exprs, span);

    let poll_futures = future_names.iter().map(|fut| {
        quote! {
            __all_done &= #futures_crate::core_reexport::future::Future::poll(
                unsafe { #futures_crate::core_reexport::pin::Pin::new_unchecked(&mut #fut) }, __cx).is_ready();
        }
    });
    let take_outputs = future_names.iter().map(|fut| {
        quote! {
            unsafe { #futures_crate::core_reexport::pin::Pin::new_unchecked(&mut #fut) }.take_output().unwrap(),
        }
    });

    TokenStream::from(quote! { {
        #( #future_let_bindings )*

        #futures_crate::future::poll_fn(move |__cx: &mut #futures_crate::task::Context<'_>| {
            let mut __all_done = true;
            #( #poll_futures )*
            if __all_done {
                #futures_crate::core_reexport::task::Poll::Ready((
                    #( #take_outputs )*
                ))
            } else {
                #futures_crate::core_reexport::task::Poll::Pending
            }
        }).await
    } })
}

/// The `try_join!` macro.
pub(crate) fn try_join(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as Join);

    let futures_crate = parsed
        .futures_crate_path
        .unwrap_or_else(|| parse_quote!(::futures_util));

    // should be def_site, but that's unstable
    let span = Span::call_site();

    let (future_let_bindings, future_names) = bind_futures(&futures_crate, parsed.fut_exprs, span);

    let poll_futures = future_names.iter().map(|fut| {
        quote! {
            if #futures_crate::core_reexport::future::Future::poll(
                unsafe { #futures_crate::core_reexport::pin::Pin::new_unchecked(&mut #fut) }, __cx).is_pending()
            {
                __all_done = false;
            } else if unsafe { #futures_crate::core_reexport::pin::Pin::new_unchecked(&mut #fut) }.output_mut().unwrap().is_err() {
                // `.err().unwrap()` rather than `.unwrap_err()` so that we don't introduce
                // a `T: Debug` bound.
                return #futures_crate::core_reexport::task::Poll::Ready(
                    #futures_crate::core_reexport::result::Result::Err(
                        unsafe { #futures_crate::core_reexport::pin::Pin::new_unchecked(&mut #fut) }.take_output().unwrap().err().unwrap()
                    )
                );
            }
        }
    });
    let take_outputs = future_names.iter().map(|fut| {
        quote! {
            // `.ok().unwrap()` rather than `.unwrap()` so that we don't introduce
            // an `E: Debug` bound.
            unsafe { #futures_crate::core_reexport::pin::Pin::new_unchecked(&mut #fut) }.take_output().unwrap().ok().unwrap(),
        }
    });

    TokenStream::from(quote! { {
        #( #future_let_bindings )*

        #futures_crate::future::poll_fn(move |__cx: &mut #futures_crate::task::Context<'_>| {
            let mut __all_done = true;
            #( #poll_futures )*
            if __all_done {
                #futures_crate::core_reexport::task::Poll::Ready(
                    #futures_crate::core_reexport::result::Result::Ok((
                        #( #take_outputs )*
                    ))
                )
            } else {
                #futures_crate::core_reexport::task::Poll::Pending
            }
        }).await
    } })
}
