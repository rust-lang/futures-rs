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
    println!("t1: {}", output);
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
    // let mut always_false = futures_await_runtime();
    // always_false.segments.push(segment("always_false"));
    // let always_false = Box::new(Expr {
    //     attrs: Vec::new(),
    //     node: ExprKind::Path(None, always_false),
    // });
    let expr_false = Box::new(Expr {
        attrs: Vec::new(),
        node: ExprKind::Lit(Lit::Bool(false)),
        // node: ExprKind::Call(always_false, Vec::new()),
    });
    let loop_expr = Box::new(Expr {
        attrs: Vec::new(),
        node: ExprKind::Loop(Block { stmts: Vec::new() }, None),
    });
    let yield_block = Block {
        stmts: vec![Stmt::Expr(Box::new(Expr {
            attrs: Vec::new(),
            node: ExprKind::Yield(Some(loop_expr)),
        }))],
    };
    let mut stmts = mem::replace(&mut block.stmts, Vec::new());
    stmts.insert(0, Stmt::Expr(Box::new(Expr {
        attrs: vec![Attribute {
            style: AttrStyle::Inner,
            path: Path {
                global: false,
                segments: vec![segment("allow")],
            },
            tts: vec![TokenTree::Delimited(Delimited {
                delim: DelimToken::Paren,
                tts: vec![TokenTree::Token(Token::Ident("unreachable_code".into()))],
            })],
            is_sugared_doc: false,
        }],
        // attrs: Vec::new(),
        node: ExprKind::If(expr_false, yield_block, None),
    })));

    let mut gen = futures_await_runtime();
    gen.segments.push(segment("gen"));
    let gen = Box::new(Expr {
        attrs: Vec::new(),
        node: ExprKind::Path(None, gen),
    });

    let decl = Box::new(FnDecl {
        inputs: Vec::new(),
        output: FunctionRetTy::Default,
        variadic: false,
    });
    let closure = Box::new(Expr {
        attrs: Vec::new(),
        node: ExprKind::Closure(CaptureBy::Value, decl, Box::new(Expr {
            attrs: Vec::new(),
            node: ExprKind::Block(Unsafety::Normal, Block { stmts: stmts }),
        })),
    });
    let closure = Box::new(Expr {
        attrs: Vec::new(),
        node: ExprKind::Paren(closure),
    });

    let generator = Expr {
        attrs: Vec::new(),
        node: ExprKind::Call(closure, Vec::new()),
    };

    block.stmts.push(Stmt::Expr(Box::new(Expr {
        attrs: Vec::new(),
        node: ExprKind::Call(gen, vec![generator]),
    })));
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

// fn futures() -> Path {
//     let mut p = futures_await_runtime();
//     p.segments.push(segment("futures"));
//     return p
// }

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
