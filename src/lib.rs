#![feature(proc_macro)]

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn async(attribute: TokenStream, function: TokenStream) -> TokenStream {
    if attribute.to_string() != "" {
        panic!("the #[async] attribute currently takes no arguments");
    }
    let mut ast = syn::parse_item(&function.to_string())
            .expect("failed to parse item");

    let function = match attr.node {
        ItemKind::Fn(ref
    };
    println!("{:?}", ast);
    function
}

#[proc_macro]
pub fn await(t: TokenStream) -> TokenStream {
    println!("t2: {}", t.to_string());
    t
}
