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
    let ref inputs = inputs;
    let output = match output {
        FunctionRetTy::Default => Ty::Tup(Vec::new()),
        FunctionRetTy::Ty(t) => t,
    };
    let ref bindings = inputs.iter().enumerate().map(|(i, input)| {
        match *input {
            FnArg::Captured(_, ref ty) => {
                let pat = Pat::Ident(BindingMode::ByValue(Mutability::Immutable),
                                     Ident::from(format!("__arg_{}", i)),
                                     None);
                FnArg::Captured(pat, ty.clone())
            }
            ref other => other.clone()
        }
    }).collect::<Vec<_>>();
    let binding_names = bindings.iter().filter_map(|input| {
        match *input {
            FnArg::Captured(Pat::Ident(_, ref name, _), _) => Some(name),
            _ => None,
        }
    }).collect::<Vec<_>>();
    let block = ExpandAsyncFor.fold_block(*block);
    assert!(!variadic, "variadic functions cannot be async");

    // Actual #[async] transformation
    let generator_name = Ident::from(format!("__{}_generator", ident));
    let maybe_self = if binding_names.len() == bindings.len() {
        None
    } else {
        Some(Ident::from("self.")) // a bit jank...
    };
    let output = quote! {
        #(#attrs)*
        #vis #unsafety #abi #constness
        fn #ident #generics(#(#bindings),*)
            -> Box<::futures_await::Future<
                    Item = <#output as ::futures_await::FutureType>::Item,
                    Error = <#output as ::futures_await::FutureType>::Error,
               >>
        {
            Box::new({
                extern crate futures_await;
                futures_await::gen
            }(#maybe_self #generator_name(#(#binding_names),*)))
        }

        #unsafety #constness fn #generator_name #generics(#(#inputs),*)
            -> impl ::futures_await::Generator<Yield = (), Return = #output>
        {
            // Ensure that this closure is a generator, even if it doesn't
            // have any `yield` statements.
            #[allow(unreachable_code)]
            {
                if false {
                    yield
                }
            }

            #block
        }
    };
    output.parse().unwrap()
}

struct ExpandAsyncFor;

impl Folder for ExpandAsyncFor {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        if expr.attrs.len() != 1 {
            return expr
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
                                futures_await::Some(e) => e,
                                futures_await::None => break,
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
}
