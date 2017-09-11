//! Procedural macro for the `#[async]` attribute.
//!
//! This crate is an implementation of the `#[async]` attribute as a procedural
//! macro. This is nightly-only for now as it's using the unstable features of
//! procedural macros. Furthermore it's generating code that's using a new
//! keyword, `yield`, and a new construct, generators, both of which are also
//! unstable.
//!
//! Currently this crate depends on `syn` and `quote` to do all the heavy
//! lifting, this is just a very small shim around creating a closure/future out
//! of a generator.

#![feature(proc_macro)]
#![recursion_limit = "128"]

extern crate proc_macro2;
extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro2::Span;
use proc_macro::{TokenStream, TokenTree, Delimiter, TokenNode};
use quote::{Tokens, ToTokens};
use syn::*;
use syn::fold::Folder;

#[proc_macro_attribute]
pub fn async(attribute: TokenStream, function: TokenStream) -> TokenStream {
    // Handle arguments to the #[async] attribute, if any
    let attribute = attribute.to_string();
    let boxed = if attribute == "( boxed )" {
        true
    } else if attribute == "" {
        false
    } else {
        panic!("the #[async] attribute currently only takes `boxed` as an arg");
    };

    // Parse our item, expecting a function. This function may be an actual
    // top-level function or it could be a method (typically dictated by the
    // arguments). We then extract everything we'd like to use.
    let Item { attrs, node } = syn::parse(function)
        .expect("failed to parse tokens as a function");
    let ItemFn {
        ident,
        vis,
        unsafety,
        constness,
        abi,
        block,
        decl,
        ..
    } = match node {
        ItemKind::Fn(item) => item,
        _ => panic!("#[async] can only be applied to functions"),
    };
    let FnDecl {
        inputs,
        output,
        variadic,
        generics,
        fn_token,
        ..
    } = { *decl };
    let where_clause = &generics.where_clause;
    assert!(!variadic, "variadic functions cannot be async");
    let (output, rarrow_token) = match output {
        FunctionRetTy::Ty(t, rarrow_token) => (t, rarrow_token),
        FunctionRetTy::Default => {
            (TyTup {
                tys: Default::default(),
                lone_comma: Default::default(),
                paren_token: Default::default(),
            }.into(), Default::default())
        }
    };

    // We've got to get a bit creative with our handling of arguments. For a
    // number of reasons we translate this:
    //
    //      fn foo(ref a: u32) -> Result<u32, u32> {
    //          // ...
    //      }
    //
    // into roughly:
    //
    //      fn foo(__arg_0: u32) -> impl Future<...> {
    //          gen(move || {
    //              let ref a = __arg0;
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
        let input = input.into_item();

        // `self: Box<Self>` will get captured naturally
        let mut is_input_no_pattern = false;
        if let FnArg::Captured(ref arg) = input {
            if let Pat::Ident(PatIdent { ref ident, ..}) = arg.pat {
                if ident == "self" {
                    is_input_no_pattern = true;
                }
            }
        }
        if is_input_no_pattern {
            inputs_no_patterns.push(input);
            continue
        }

        match input {
            FnArg::Captured(ArgCaptured {
                pat: syn::Pat::Ident(syn::PatIdent {
                    mode: BindingMode::ByValue(_),
                    ..
                }),
                ..
            }) => {
                inputs_no_patterns.push(input);
            }

            // `ref a: B` (or some similar pattern)
            FnArg::Captured(ArgCaptured { pat, ty, colon_token }) => {
                patterns.push(pat);
                let ident = Ident::from(format!("__arg_{}", i));
                temp_bindings.push(ident.clone());
                let pat = PatIdent {
                    mode: BindingMode::ByValue(Mutability::Immutable),
                    ident: ident,
                    at_token: None,
                    subpat: None,
                };
                inputs_no_patterns.push(ArgCaptured {
                    pat: pat.into(),
                    ty,
                    colon_token,
                }.into());
            }

            // Other `self`-related arguments get captured naturally
            _ => {
                inputs_no_patterns.push(input);
            }
        }
    }

    // This is the point where we handle
    //
    //      #[async]
    //      for x in y {
    //      }
    //
    // Basically just take all those expression and expand them.
    let block = ExpandAsyncFor.fold_block(*block);

    // TODO: can we lift the restriction that `futures` must be at the root of
    //       the crate?

    let output_span = first_last(&output);
    let return_ty = if boxed {
        quote! {
            Box<::futures::Future<
                Item = <! as ::futures::__rt::IsResult>::Ok,
                Error = <! as ::futures::__rt::IsResult>::Err,
            >>
        }
    } else {
        // Dunno why this is buggy, hits weird typecheck errors in tests
        //
        // quote! {
        //     impl ::futures::Future<
        //         Item = <#output as ::futures::__rt::MyTry>::MyOk,
        //         Error = <#output as ::futures::__rt::MyTry>::MyError,
        //     >
        // }
        quote! { impl ::futures::__rt::MyFuture<!> + 'static }
    };
    let return_ty = respan(return_ty.into(), &output_span);
    let return_ty = replace_bang(return_ty, &output);

    let block_inner = quote! {
        #( let #patterns = #temp_bindings; )*
        #block
    };
    let mut result = Tokens::new();
    block.brace_token.surround(&mut result, |tokens| {
        block_inner.to_tokens(tokens);
    });
    syn::tokens::Semi([block.brace_token.0]).to_tokens(&mut result);

    let gen_body_inner = quote! {
        let __e: ::futures::__rt::Result<_,_> = #result

        // Ensure that this closure is a generator, even if it doesn't
        // have any `yield` statements.
        #[allow(unreachable_code)]
        {
            return __e;
            loop { yield }
        }
    };
    let mut gen_body = Tokens::new();
    block.brace_token.surround(&mut gen_body, |tokens| {
        gen_body_inner.to_tokens(tokens);
    });

    // Give the invocation of the `gen` function the same span as the output
    // as currently errors related to it being a result are targeted here. Not
    // sure if more errors will highlight this function call...
    let gen_function = quote! { ::futures::__rt::gen };
    let gen_function = respan(gen_function.into(), &output_span);
    let body_inner = quote! {
        #gen_function (move || #gen_body)
    };
    let body_inner = if boxed {
        let body = quote! { Box::new(#body_inner) };
        respan(body.into(), &output_span)
    } else {
        body_inner.into()
    };
    let mut body = Tokens::new();
    block.brace_token.surround(&mut body, |tokens| {
        body_inner.to_tokens(tokens);
    });

    let output = quote! {
        #(#attrs)*
        #vis #unsafety #abi #constness
        #fn_token #ident #generics(#(#inputs_no_patterns),*)
            #rarrow_token #return_ty
            #where_clause
        #body
    };

    // println!("{}", output);
    output.into()
}

#[proc_macro]
pub fn async_block(input: TokenStream) -> TokenStream {
    let input = TokenStream::from(TokenTree {
        kind: TokenNode::Group(Delimiter::Brace, input),
        span: Default::default(),
    });
    let expr = syn::parse(input)
        .expect("failed to parse tokens as an expression");
    let expr = ExpandAsyncFor.fold_expr(expr);

    let mut tokens = quote! {
        ::futures::__rt::gen
    };
    let span = syn::Span(Span::call_site());
    syn::tokens::Paren(span).surround(&mut tokens, |tokens| {
        syn::tokens::Move(span).to_tokens(tokens);
        syn::tokens::OrOr([span, span]).to_tokens(tokens);
        syn::tokens::Brace(span).surround(tokens, |tokens| {
            (quote! { if false { yield } }).to_tokens(tokens);
            expr.to_tokens(tokens);
        });
    });

    tokens.into()
}

struct ExpandAsyncFor;

impl Folder for ExpandAsyncFor {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        let expr = fold::noop_fold_expr(self, expr);
        if expr.attrs.len() != 1 {
            return expr
        }
        // TODO: more validation here
        if expr.attrs[0].path.segments.get(0).item().ident != "async" {
            return expr
        }
        let all = match expr.node {
            ExprKind::ForLoop(item) => item,
            _ => panic!("only for expressions can have #[async]"),
        };
        let ExprForLoop { pat, expr, body, label, colon_token, .. } = all;

        // Basically just expand to a `poll` loop
        let tokens = quote! {{
            let mut __stream = #expr;
            #label
            #colon_token
            loop {
                let #pat = {
                    extern crate futures_await;
                    let r = futures_await::Stream::poll(&mut __stream)?;
                    match r {
                        futures_await::Async::Ready(e) => {
                            match e {
                                futures_await::__rt::Some(e) => e,
                                futures_await::__rt::None => break,
                            }
                        }
                        futures_await::Async::NotReady => {
                            yield;
                            continue
                        }
                    }
                };

                #body
            }
        }};
        syn::parse(tokens.into()).unwrap()
    }

    // Don't recurse into items
    fn fold_item(&mut self, item: Item) -> Item {
        item
    }
}

fn first_last(tokens: &ToTokens) -> (Span, Span) {
    let mut spans = Tokens::new();
    tokens.to_tokens(&mut spans);
    let good_tokens = proc_macro2::TokenStream::from(spans).into_iter().collect::<Vec<_>>();
    let first_span = good_tokens.first().map(|t| t.span).unwrap_or(Default::default());
    let last_span = good_tokens.last().map(|t| t.span).unwrap_or(first_span);
    (first_span, last_span)
}

fn respan(input: proc_macro2::TokenStream,
          &(first_span, last_span): &(Span, Span)) -> proc_macro2::TokenStream {
    let mut new_tokens = input.into_iter().collect::<Vec<_>>();
    if let Some(token) = new_tokens.first_mut() {
        token.span = first_span;
    }
    for token in new_tokens.iter_mut().skip(1) {
        token.span = last_span;
    }
    new_tokens.into_iter().collect()
}

fn replace_bang(input: proc_macro2::TokenStream, tokens: &ToTokens)
    -> proc_macro2::TokenStream
{
    let mut new_tokens = Tokens::new();
    for token in input.into_iter() {
        match token.kind {
            proc_macro2::TokenNode::Op('!', _) => tokens.to_tokens(&mut new_tokens),
            _ => token.to_tokens(&mut new_tokens),
        }
    }
    new_tokens.into()
}
