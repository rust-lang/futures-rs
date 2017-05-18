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
    let ast = syn::parse_item(&function.to_string())
                    .expect("failed to parse item");
    let Item { ident, vis, attrs, node } = ast;
    let all = match node {
        ItemKind::Fn(a, b, c, d, e, f) => (a, b, c, d, e, f),
        _ => panic!("#[async] can only be applied to functions"),
    };
    let (decl, unsafety, constness, abi, generics, block) = all;
    let where_clause = &generics.where_clause;
    let FnDecl { inputs, output, variadic } = { *decl };
    assert!(!variadic, "variadic functions cannot be async");
    let ref inputs = inputs;
    let output = match output {
        FunctionRetTy::Default => Ty::Tup(Vec::new()),
        FunctionRetTy::Ty(t) => t,
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
        match *input {
            // `self: Box<Self>` will get captured naturally
            FnArg::Captured(Pat::Ident(_, ref ident, _), _) if ident == "self" => {
                inputs_no_patterns.push(input.clone());
            }

            // `a: B`
            FnArg::Captured(ref pat, ref ty) => {
                patterns.push(pat);
                let ident = Ident::from(format!("__arg_{}", i));
                temp_bindings.push(ident.clone());
                let pat = Pat::Ident(BindingMode::ByValue(Mutability::Immutable),
                                     ident,
                                     None);
                inputs_no_patterns.push(FnArg::Captured(pat, ty.clone()));
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
            #maybe_boxed (::futures::__rt::gen((move || {
                let __e = {
                    #( let #patterns = #temp_bindings; )*
                    #block
                };

                // Ensure that this closure is a generator, even if it doesn't
                // have any `yield` statements.
                #[allow(unreachable_code)]
                {
                    return __e;
                    if false {
                        yield
                    }
                    loop {}
                }
            })()))
        }
    };

    // println!("{}", output);
    output.parse().unwrap()
}

struct ExpandAsyncFor;

impl Folder for ExpandAsyncFor {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        if expr.attrs.len() != 1 {
            return fold::noop_fold_expr(self, expr)
        }
        // TODO: more validation here
        if expr.attrs[0].path.segments[0].ident != "async" {
            return expr
        }
        let all = match expr.node {
            ExprKind::ForLoop(a, b, c, d) => (a, b, c, d),
            _ => panic!("only for expressions can have #[async]"),
        };
        let (pat, expr, block, ident) = all;

        // Basically just expand to a `poll` loop
        let tokens = quote! {{
            let mut __stream = #expr;
            #ident
            loop {
                let #pat = {
                    extern crate futures_await;
                    match futures_await::Stream::poll(&mut __stream)? {
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

                #block
            }
        }};
        parse_expr(tokens.as_str()).unwrap()
    }

    // Don't recurse into items
    fn fold_item(&mut self, item: Item) -> Item {
        item
    }
}
