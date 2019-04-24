//! Procedural macro for the `#[async_stream]` attribute.

#![recursion_limit = "128"]
#![warn(rust_2018_idioms)]

extern crate proc_macro;

use proc_macro::{Delimiter, Group, TokenStream, TokenTree};
use proc_macro2::{Span, TokenStream as TokenStream2, TokenTree as TokenTree2};
use quote::{quote, ToTokens};
use syn::{
    fold::{self, Fold},
    token, ArgCaptured, Error, Expr, ExprForLoop, ExprMacro, FnArg, FnDecl, Ident, Item, ItemFn,
    Pat, PatIdent, ReturnType, TypeTuple,
};

#[macro_use]
mod error;

mod elision;

#[proc_macro_attribute]
pub fn for_await(args: TokenStream, input: TokenStream) -> TokenStream {
    assert_!(args.is_empty(), args_is_not_empty!("for_await"));

    let mut expr: ExprForLoop = syn::parse_macro_input!(input);
    expr.attrs.push(syn::parse_quote!(#[for_await]));
    Expand(Future).fold_expr(Expr::ForLoop(expr)).into_token_stream().into()
}

#[proc_macro_attribute]
pub fn async_stream(args: TokenStream, input: TokenStream) -> TokenStream {
    assert_!(args.is_empty(), args_is_not_empty!("async_stream"));

    let item: ItemFn = syn::parse_macro_input!(input);
    expand_async_stream_fn(item)
}

fn expand_async_stream_fn(item: ItemFn) -> TokenStream {
    // Parse our item, expecting a function. This function may be an actual
    // top-level function or it could be a method (typically dictated by the
    // arguments). We then extract everything we'd like to use.
    let ItemFn { ident, vis, constness, unsafety, abi, block, decl, attrs, .. } = item;
    let FnDecl { inputs, output, variadic, mut generics, fn_token, .. } = *decl;
    let where_clause = &generics.where_clause;
    assert_!(variadic.is_none(), "variadic functions cannot be async");
    let (output, rarrow_token) = match output {
        ReturnType::Type(rarrow_token, t) => (*t, rarrow_token),
        ReturnType::Default => (
            TypeTuple { elems: Default::default(), paren_token: Default::default() }.into(),
            Default::default(),
        ),
    };

    // We've got to get a bit creative with our handling of arguments. For a
    // number of reasons we translate this:
    //
    //      fn foo(ref a: u32) -> u32 {
    //          // ...
    //      }
    //
    // into roughly:
    //
    //      fn foo(__arg_0: u32) -> impl Stream<Item = u32> {
    //          from_generator(static move || {
    //              let ref a = __arg_0;
    //
    //              // ...
    //          })
    //      }
    //
    // The intention here is to ensure that all local function variables get
    // moved into the generator we're creating, and they're also all then bound
    // appropriately according to their patterns and whatnot.
    //
    // We notably skip everything related to `self` which typically doesn't have
    // many patterns with it and just gets captured naturally.
    let mut inputs_no_patterns = Vec::new();
    let mut patterns = Vec::new();
    let mut temp_bindings = Vec::new();
    for (i, input) in inputs.into_iter().enumerate() {
        // `self: Box<Self>` will get captured naturally
        let mut is_input_no_pattern = false;
        if let FnArg::Captured(ArgCaptured { pat: Pat::Ident(pat), .. }) = &input {
            if pat.ident == "self" {
                is_input_no_pattern = true;
            }
        }
        if is_input_no_pattern {
            inputs_no_patterns.push(input);
            continue;
        }

        if let FnArg::Captured(ArgCaptured {
            pat: pat @ Pat::Ident(PatIdent { by_ref: Some(_), .. }),
            ty,
            colon_token,
        }) = input
        {
            // `ref a: B` (or some similar pattern)
            patterns.push(pat);
            let ident = Ident::new(&format!("__arg_{}", i), Span::call_site());
            temp_bindings.push(ident.clone());
            let pat = PatIdent { by_ref: None, mutability: None, ident, subpat: None }.into();
            inputs_no_patterns.push(ArgCaptured { pat, ty, colon_token }.into());
        } else {
            // Other arguments get captured naturally
            inputs_no_patterns.push(input);
        }
    }

    // This is the point where we handle
    //
    //      #[for_await]
    //      for x in y {
    //      }
    //
    // Basically just take all those expression and expand them.
    //
    // Also, in some items, it needs to adjust the type to be generated depending on whether it is
    // called in the scope of async or the scope of async-stream, it is processed here.
    let block = Expand(Stream).fold_block(*block);

    let block_inner = quote! {
        #( let #patterns = #temp_bindings; )*
        #block
    };
    let mut result = TokenStream2::new();
    block.brace_token.surround(&mut result, |tokens| {
        block_inner.to_tokens(tokens);
    });
    token::Semi([block.brace_token.span]).to_tokens(&mut result);

    let gen_body_inner = quote! {
        let (): () = #result

        // Ensure that this closure is a generator, even if it doesn't
        // have any `yield` statements.
        #[allow(unreachable_code)]
        {
            return;
            loop { yield ::futures::core_reexport::task::Poll::Pending }
        }
    };
    let mut gen_body = TokenStream2::new();
    block.brace_token.surround(&mut gen_body, |tokens| {
        gen_body_inner.to_tokens(tokens);
    });

    // Give the invocation of the `from_generator` function the same span as the output
    // as currently errors related to it being a result are targeted here. Not
    // sure if more errors will highlight this function call...
    let output_span = first_last(&output);
    let gen_function = quote! { ::futures::async_stream::from_generator };
    let gen_function = respan(gen_function, output_span);
    let body_inner = quote! {
        #gen_function (static move || -> () #gen_body)
    };
    let mut body = TokenStream2::new();
    block.brace_token.surround(&mut body, |tokens| {
        body_inner.to_tokens(tokens);
    });

    let inputs_no_patterns = elision::unelide_lifetimes(&mut generics.params, inputs_no_patterns);
    let lifetimes: Vec<_> = generics.lifetimes().map(|l| &l.lifetime).collect();

    // Raw `impl` breaks syntax highlighting in some editors.
    let impl_token = token::Impl::default();
    let return_ty = quote! {
        #impl_token ::futures::stream::Stream<Item = #output> + #(#lifetimes +)*
    };
    let return_ty = respan(return_ty, output_span);
    TokenStream::from(quote! {
        #(#attrs)*
        #vis #unsafety #abi #constness
        #fn_token #ident #generics (#(#inputs_no_patterns),*)
            #rarrow_token #return_ty
            #where_clause
        #body
    })
}

#[proc_macro]
pub fn async_stream_block(input: TokenStream) -> TokenStream {
    let input = TokenStream::from(TokenTree::Group(Group::new(Delimiter::Brace, input)));
    let expr = syn::parse_macro_input!(input);
    let expr = Expand(Stream).fold_expr(expr);

    let mut tokens = quote! { ::futures::async_stream::from_generator };

    // Use some manual token construction here instead of `quote!` to ensure
    // that we get the `call_site` span instead of the default span.
    let span = Span::call_site();
    token::Paren(span).surround(&mut tokens, |tokens| {
        token::Static(span).to_tokens(tokens);
        token::Move(span).to_tokens(tokens);
        token::OrOr([span, span]).to_tokens(tokens);
        token::Brace(span).surround(tokens, |tokens| {
            (quote! {
                if false { yield ::futures::core_reexport::task::Poll::Pending }
            })
            .to_tokens(tokens);
            expr.to_tokens(tokens);
        });
    });

    tokens.into()
}

/// The scope in which `#[for_await]`, `await!` and `await_item!` was called.
///
/// The type of generator depends on which scope is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// `async fn`, `async {}`, `async ||`
    Future,
    /// `#[async_stream]`, `async_stream_block!`
    Stream,
    /// `static move ||`, `||`
    ///
    /// It cannot call `#[for_await]`, `await!` and `await_item!` in this scope.
    Closure,
}

use Scope::{Closure, Future, Stream};

struct Expand(Scope);

impl Expand {
    /// Expands `#[for_await] for pat in expr { .. }`.
    fn expand_for_await(&self, mut expr: ExprForLoop) -> Expr {
        if !(expr.attrs.len() == 1 && expr.attrs[0].path.is_ident("for_await")) {
            return Expr::ForLoop(expr);
        } else if !expr.attrs[0].tts.is_empty() {
            return error::expr_compile_error(&Error::new_spanned(
                expr.attrs.pop(),
                args_is_not_empty!("for_await"),
            ));
        }

        // It needs to adjust the type yielded by the macro because generators used internally by
        // async fn yield `()` type, but generators used internally by `async_stream` yield
        // `Poll<U>` type.
        let yield_ = match self.0 {
            Future => TokenStream2::new(),
            Stream => quote! { ::futures::core_reexport::task::Poll::Pending },
            Closure => return outside_of_async_error!(expr, "#[for_await]"),
        };
        let ExprForLoop { label, pat, expr, body, .. } = expr;

        // Basically just expand to a `poll` loop
        syn::parse_quote! {{
            let mut __pinned = #expr;
            #label
            loop {
                let #pat = {
                    match ::futures::async_stream::poll_next_with_tls_context(unsafe {
                            ::futures::core_reexport::pin::Pin::new_unchecked(&mut __pinned)
                        })
                    {
                        ::futures::core_reexport::task::Poll::Ready(e) => {
                            match e {
                                ::futures::core_reexport::option::Option::Some(e) => e,
                                ::futures::core_reexport::option::Option::None => break,
                            }
                        }
                        ::futures::core_reexport::task::Poll::Pending => {
                            yield #yield_;
                            continue
                        }
                    }
                };

                #body
            }
        }}
    }

    /* TODO: If this is enabled, it can be used for `yield` instead of `stream_yield!`.
             Raw `yield` is simpler, but it may be preferable to distinguish it from `yield` called
             in other scopes.
    /// Expands `yield expr` in `async_stream` scope.
    fn expand_yield(&self, expr: ExprYield) -> ExprYield {
        if self.0 != Stream {
            return expr;
        }

        let ExprYield { attrs, yield_token, expr } = expr;
        let expr = expr.map_or_else(|| quote!(()), ToTokens::into_token_stream);
        let expr = syn::parse_quote! {
            ::futures::core_reexport::task::Poll::Ready(#expr)
        };
        ExprYield { attrs, yield_token, expr: Some(Box::new(expr)) }
    }
    */

    /// Expands a macro.
    fn expand_macro(&mut self, mut expr: ExprMacro) -> Expr {
        if self.0 == Stream && expr.mac.path.is_ident("await") {
            return self.expand_await_macros(expr, "poll_with_tls_context");
        } else if expr.mac.path.is_ident("await_item") {
            match self.0 {
                Stream => return self.expand_await_macros(expr, "poll_next_with_tls_context"),
                Closure => return outside_of_async_error!(expr, "await_item!"),
                Future => {}
            }
        } else if expr.mac.path.is_ident("async_stream_block") {
            // FIXME: When added Parse impl for ExprCall, replace `if let ..` + `unreachable!()`
            //        with `let` + `.unwrap()`
            if let Ok(Expr::Call(mut e)) = syn::parse(async_stream_block(expr.mac.tts.into())) {
                e.attrs.append(&mut expr.attrs);
                return Expr::Call(e);
            } else {
                unreachable!()
            }
        } else if self.0 != Stream && expr.mac.path.is_ident("stream_yield") {
            // TODO: Should we remove real `stream_yield!` macro and replace `stream_yield!` call in
            //       here? -- or should we use `yield` instead of ``?
            return outside_of_async_stream_error!(expr, "stream_yield!");
        }

        Expr::Macro(expr)
    }

    /// Expands `await!(expr)` or `await_item!(expr)` in `async_stream` scope.
    ///
    /// It needs to adjust the type yielded by the macro because generators used internally by
    /// async fn yield `()` type, but generators used internally by `async_stream` yield
    /// `Poll<U>` type.
    fn expand_await_macros(&mut self, expr: ExprMacro, poll_fn: &str) -> Expr {
        assert_eq!(self.0, Stream);

        let expr = expr.mac.tts;
        let poll_fn = Ident::new(poll_fn, Span::call_site());

        // Because macro input (`#expr`) is untrusted, use `syn::parse2` + `expr_compile_error`
        // instead of `syn::parse_quote!` to generate better error messages (`syn::parse_quote!`
        // panics if fail to parse).
        syn::parse2(quote! {{
            let mut __pinned = #expr;
            loop {
                if let ::futures::core_reexport::task::Poll::Ready(x) =
                    ::futures::async_stream::#poll_fn(unsafe {
                        ::futures::core_reexport::pin::Pin::new_unchecked(&mut __pinned)
                    })
                {
                    break x;
                }

                yield ::futures::core_reexport::task::Poll::Pending
            }
        }})
        .unwrap_or_else(|e| error::expr_compile_error(&e))
    }
}

impl Fold for Expand {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        // Backup current scope and adjust the scope. This must be done before folding expr.
        let tmp = self.0;
        match &expr {
            Expr::Async(_) => self.0 = Future,
            Expr::Closure(expr) => self.0 = if expr.asyncness.is_some() { Future } else { Closure },
            Expr::Macro(expr) if expr.mac.path.is_ident("async_stream_block") => self.0 = Stream,
            _ => {}
        }

        let expr = match fold::fold_expr(self, expr) {
            Expr::ForLoop(expr) => self.expand_for_await(expr),
            // Expr::Yield(expr) => Expr::Yield(self.expand_yield(expr)),
            Expr::Macro(expr) => self.expand_macro(expr),
            expr => expr,
        };

        // Restore the backup.
        self.0 = tmp;
        expr
    }

    // Don't recurse into items
    fn fold_item(&mut self, item: Item) -> Item {
        item
    }
}

fn first_last<T: ToTokens>(tokens: &T) -> (Span, Span) {
    let mut spans = TokenStream2::new();
    tokens.to_tokens(&mut spans);
    let good_tokens = spans.into_iter().collect::<Vec<_>>();
    let first_span = good_tokens.first().map_or_else(Span::call_site, TokenTree2::span);
    let last_span = good_tokens.last().map_or_else(|| first_span, TokenTree2::span);
    (first_span, last_span)
}

fn respan(input: TokenStream2, (first_span, last_span): (Span, Span)) -> TokenStream2 {
    let mut new_tokens = input.into_iter().collect::<Vec<_>>();
    if let Some(token) = new_tokens.first_mut() {
        token.set_span(first_span);
    }
    for token in new_tokens.iter_mut().skip(1) {
        token.set_span(last_span);
    }
    new_tokens.into_iter().collect()
}
