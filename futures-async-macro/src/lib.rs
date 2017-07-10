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

extern crate proc_macro;
extern crate proc_macro2;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;
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
    let FnDecl { inputs, output, variadic, generics, .. } = { *decl };
    let where_clause = &generics.where_clause;
    assert!(!variadic, "variadic functions cannot be async");
    let ref inputs = inputs;
    let output = match output {
        FunctionRetTy::Ty(t, _) => t,
        FunctionRetTy::Default => {
            TyTup {
                tys: Default::default(),
                lone_comma: Default::default(),
                paren_token: Default::default(),
            }.into()
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
    for (i, input) in inputs.iter().enumerate() {
        let input = *input.item();

        // `self: Box<Self>` will get captured naturally
        if let FnArg::Captured(ref arg) = *input {
            if let Pat::Ident(PatIdent { ref ident, ..}) = arg.pat {
                if ident == "self" {
                    inputs_no_patterns.push(input.clone());
                    continue
                }
            }
        }

        match *input {
            // `a: B`
            FnArg::Captured(ArgCaptured { ref pat, ref ty, .. }) => {
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
                    ty: ty.clone(),
                    colon_token: Default::default(),
                }.into());
            }

            // Other `self`-related arguments get captured naturally
            _ => {
                inputs_no_patterns.push(input.clone());
            }
        }
    }

    // This is the point where we Handle
    //
    //      #[async]
    //      for x in y {
    //      }
    //
    // Basically just take all those expression and expand them.
    let block = ExpandAsyncFor.fold_block(*block);

    // TODO: can we lift the restriction that `futures` must be at the root of
    //       the crate?
    let return_ty = if boxed {
        quote! {
            Box<::futures::Future<
                Item = <#output as ::futures::__rt::FutureType>::Item,
                Error = <#output as ::futures::__rt::FutureType>::Error,
            >>
        }
    } else {
        // Dunno why this is buggy, hits weird typecheck errors in
        // `examples/main.rs`
        // -> impl ::futures::Future<
        //         Item = <#output as ::futures::__rt::FutureType>::Item,
        //         Error = <#output as ::futures::__rt::FutureType>::Error,
        //    >

        quote! { impl ::futures::__rt::MyFuture<#output> }
    };

    let maybe_boxed = if boxed {
        quote! { Box::new }
    } else {
        quote! { }
    };

    let output = quote! {
        #(#attrs)*
        #vis #unsafety #abi #constness
        fn #ident #generics(#(#inputs_no_patterns),*) -> #return_ty
            #where_clause
        {
            #maybe_boxed (::futures::__rt::gen(move || {
                let __e = {
                    #( let #patterns = #temp_bindings; )*
                    #block
                };

                // Ensure that this closure is a generator, even if it doesn't
                // have any `yield` statements.
                #[allow(unreachable_code)]
                {
                    return __e;
                    loop { yield }
                }
            }))
        }
    };

    // println!("{}", output);
    output.into()
}

struct ExpandAsyncFor;

impl Folder for ExpandAsyncFor {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        if expr.attrs.len() != 1 {
            return fold::noop_fold_expr(self, expr)
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
