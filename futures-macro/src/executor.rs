use proc_macro::TokenStream;
use quote::quote;

pub(crate) fn test(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(item as syn::ItemFn);
    let attrs = &input.attrs;
    let vis = &input.vis;
    let sig = &mut input.sig;
    let body = &input.block;

    if sig.asyncness.is_none() {
        return syn::Error::new_spanned(sig.fn_token, "Only async functions are supported")
            .to_compile_error()
            .into();
    }

    sig.asyncness = None;

    let gen = quote! {
        #(#attrs)*
        #vis #sig {
            ::futures_test::__private::block_on(async move { #body })
        }
    };

    gen.into()
}
