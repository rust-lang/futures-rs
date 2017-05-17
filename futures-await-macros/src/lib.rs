#![feature(proc_macro)]
#![recursion_limit = "128"]

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;
use syn::*;

#[proc_macro_attribute]
pub fn async(attribute: TokenStream, function: TokenStream) -> TokenStream {
    if attribute.to_string() != "" {
        panic!("the #[async] attribute currently takes no arguments");
    }

    let ast = syn::parse_item(&function.to_string())
                    .expect("failed to parse item");
    let Item { ident, vis, attrs, node } = ast;
    let all = match node {
        ItemKind::Fn(a, b, c, d, e, f) => (a, b, c, d, e, f),
        _ => panic!("#[async] can only be applied to functions"),
    };
    let (decl, unsafety, constness, abi, generics, block) = all;
    let FnDecl { inputs, output, variadic } = { *decl };
    let output = match output {
        FunctionRetTy::Default => Ty::Tup(Vec::new()),
        FunctionRetTy::Ty(t) => t,
    };
    assert!(!variadic, "variadic functions cannot be async");

    // Actual #[async] transformation
    let output = quote! {
        #(#attrs)*
        #vis #unsafety #abi #constness fn #ident #generics(#(#inputs),*)
            -> Box<::futures_await::Future<
                    Item = <#output as ::futures_await::FutureType>::Item,
                    Error = <#output as ::futures_await::FutureType>::Error,
               >>
        {
            Box::new({
                extern crate futures_await;
                futures_await::gen
            }((move || {
                // Ensure that this closure is a generator, even if it doesn't
                // have any `yield` statements.
                #[allow(unreachable_code)]
                {
                    if false {
                        yield
                    }
                }

                #block
            })()))
        }
    };
    output.parse().unwrap()
}

#[proc_macro]
pub fn await(future: TokenStream) -> TokenStream {
    let future = syn::parse_expr(&future.to_string())
                    .expect("failed to parse expression");

    // Basically just expand to a `poll` loop
    let tokens = quote! {{
        let mut future = #future;
        {
            extern crate futures_await;
            let ret;
            loop {
                match futures_await::Future::poll(&mut future) {
                    futures_await::Ok(futures_await::Async::Ready(e)) => {
                        ret = futures_await::Ok(e);
                        break
                    }
                    futures_await::Ok(futures_await::Async::NotReady) => {
                        yield
                    }
                    futures_await::Err(e) => {
                        ret = futures_await::Err(e);
                        break
                    }
                }
            }
            ret
        }
    }};
    tokens.parse().unwrap()
}
