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
    let where_clause = &generics.where_clause;
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
    let ref inputs_mapped = inputs.iter().map(|input| {
        match *input {
            FnArg::Captured(Pat::Ident(ref mutability,
                                       ref ident,
                                       ref pat),
                            ref ty) => {
                let ident = if ident == "self" {
                    Ident::from("__self")
                } else {
                    ident.clone()
                };
                FnArg::Captured(Pat::Ident(mutability.clone(), ident, pat.clone()),
                                ty.clone())
            }
            ref cap @ FnArg::Captured(..) => cap.clone(),
            FnArg::Ignored(_) => panic!("can't work with ignored fn args"),
            FnArg::SelfRef(..) => panic!("self reference async methods are unsound"),
            FnArg::SelfValue(mutability) => {
                let me = Path {
                    global: false,
                    segments: vec![PathSegment {
                        ident: Ident::from("Self"),
                        parameters: PathParameters::AngleBracketed(Default::default()),
                    }],
                };
                FnArg::Captured(Pat::Ident(BindingMode::ByValue(mutability),
                                           Ident::from("__self"),
                                           None),
                                Ty::Path(None, me))
            }
        }
    }).collect::<Vec<_>>();
    let binding_names = bindings.iter().map(|input| {
        match *input {
            FnArg::Captured(Pat::Ident(_, ref name, _), _) => name.clone(),
            _ => Ident::from("self"),
        }
    }).collect::<Vec<_>>();
    let block = ExpandAsyncFor.fold_block(*block);
    let block = RewriteSelfReferences.fold_block(block);
    assert!(!variadic, "variadic functions cannot be async");

    // Actual #[async] transformation
    let output = quote! {
        #(#attrs)*
        #vis #unsafety #abi #constness
        fn #ident #generics(#(#bindings),*)
            // Dunno why this is buggy, hits an ICE when compiling
            // `examples/main.rs`
            // -> impl ::futures::__rt::MyFuture<#output>

            // Dunno why this is buggy, hits an ICE when compiling
            // `examples/main.rs`
            // -> impl ::futures::__rt::Future<
            //         Item = <#output as ::futures::__rt::FutureType>::Item,
            //         Error = <#output as ::futures::__rt::FutureType>::Error,
            //    >

            -> Box<::futures::Future<
                    Item = <#output as ::futures::__rt::FutureType>::Item,
                    Error = <#output as ::futures::__rt::FutureType>::Error,
               >>
            #where_clause
        {
            Box::new(::futures::__rt::gen((|#(#inputs_mapped),*| {
                // Ensure that this closure is a generator, even if it doesn't
                // have any `yield` statements.
                #[allow(unreachable_code)]
                {
                    if false {
                        yield
                    }
                }

                #block
            })(#(#binding_names),*)))
        }
    };
    // println!("{}", output);
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

struct RewriteSelfReferences;

impl Folder for RewriteSelfReferences {
    fn fold_path(&mut self, mut path: Path) -> Path {
        if path.segments.len() == 1 && !path.global &&
            path.segments[0].ident == "self" {
            path.segments[0].ident = Ident::from("__self");
        }
        path
    }

    // Don't recurse into items
    fn fold_item(&mut self, item: Item) -> Item {
        item
    }
}
