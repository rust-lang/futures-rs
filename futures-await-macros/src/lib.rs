#![feature(proc_macro)]

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use std::mem;

use proc_macro::TokenStream;
use syn::*;

#[proc_macro_attribute]
pub fn async(attribute: TokenStream, function: TokenStream) -> TokenStream {
    if attribute.to_string() != "" {
        panic!("the #[async] attribute currently takes no arguments");
    }
    let mut ast = syn::parse_item(&function.to_string())
            .expect("failed to parse item");

    match ast.node {
        ItemKind::Fn(ref mut decl, _, _, _, _, ref mut block) => {
            asyncify(decl, block)
        }
        _ => panic!("#[async] can only be applied to functions"),
    }
    let output = quote! { #ast };
    output.parse().unwrap()
}

fn asyncify(decl: &mut FnDecl, block: &mut Block) {
    return_impl_future(decl);
    futureify(block);
}

fn return_impl_future(decl: &mut FnDecl) {
    let bang = FunctionRetTy::Ty(Ty::Never);
    let return_ty = match mem::replace(&mut decl.output, bang) {
        FunctionRetTy::Default => panic!("#[async] functions must return a result"),
        FunctionRetTy::Ty(t) => t,
    };

    // let (item, error) = {
    //     let mut ty_as_future_result = QSelf {
    //         ty: Box::new(return_ty),
    //         position: 0,
    //     };
    //     let mut base = futures_await_runtime();
    //     base.segments.push(segment("FutureType"));
    //     ty_as_future_result.position = base.segments.len();
    //
    //     let mut item = base.clone();
    //     item.segments.push(segment("Item"));
    //     let item = Ty::Path(Some(ty_as_future_result.clone()), item);
    //
    //     let mut error = base;
    //     error.segments.push(segment("Error"));
    //     let error = Ty::Path(Some(ty_as_future_result), error);
    //     (item, error)
    // };
    //
    // let mut future = futures();
    // future.segments.push(PathSegment {
    //     ident: "Future".into(),
    //     parameters: PathParameters::AngleBracketed(AngleBracketedParameterData {
    //         lifetimes: Vec::new(),
    //         types: Vec::new(),
    //         bindings: vec![
    //             TypeBinding {
    //                 ident: "Item".into(),
    //                 ty: item,
    //             },
    //             TypeBinding {
    //                 ident: "Error".into(),
    //                 ty: error,
    //             },
    //         ],
    //     }),
    // });

    let mut future = futures_await_runtime();
    future.segments.push(PathSegment {
        ident: "MyFuture".into(),
        parameters: PathParameters::AngleBracketed(AngleBracketedParameterData {
            lifetimes: Vec::new(),
            bindings: Vec::new(),
            types: vec![return_ty],
        }),
    });

    // let mut future_object = futures_await_runtime();
    // future_object.segments.push(PathSegment {
    //     ident: "Box".into(),
    //     parameters: PathParameters::AngleBracketed(AngleBracketedParameterData {
    //         lifetimes: Vec::new(),
    //         types: vec![Ty::Path(None, future)],
    //         bindings: Vec::new(),
    //     }),
    // });
    // let future = Ty::Path(None, future_object);
    // decl.output = FunctionRetTy::Ty(future);

    // TODO: use impl trait, but https://is.gd/T4Doln
    let future = PolyTraitRef {
        bound_lifetimes: Vec::new(),
        trait_ref: future,
    };
    let future = TyParamBound::Trait(future, TraitBoundModifier::None);
    decl.output = FunctionRetTy::Ty(Ty::ImplTrait(vec![future]));
}

fn futureify(block: &mut Block) {
}

fn segment(ident: &str) -> PathSegment {
    let no_params = PathParameters::AngleBracketed(AngleBracketedParameterData  {
        lifetimes: Vec::new(),
        types: Vec::new(),
        bindings: Vec::new(),
    });
    PathSegment {
        ident: ident.into(),
        parameters: no_params,
    }
}

fn futures() -> Path {
    let mut p = futures_await_runtime();
    p.segments.push(segment("futures"));
    return p
}

fn futures_await_runtime() -> Path {
    Path {
        global: true,
        segments: vec![segment("futures_await")],
    }
}

#[proc_macro]
pub fn await(t: TokenStream) -> TokenStream {
    println!("t2: {}", t.to_string());
    t
}
