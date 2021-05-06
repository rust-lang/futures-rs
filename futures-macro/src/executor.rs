use proc_macro::TokenStream;
use quote::quote;

pub(crate) fn test(args: TokenStream, item: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new_spanned(args, "invalid argument"))
            .to_compile_error()
            .into();
    }
    let mut input = syn::parse_macro_input!(item as syn::ItemFn);
    let attrs = &input.attrs;
    let vis = &input.vis;
    let sig = &mut input.sig;
    let body = &input.block;

    if sig.asyncness.take().is_none() {
        return syn::Error::new_spanned(sig.fn_token, "Only async functions are supported")
            .to_compile_error()
            .into();
    }

    let gen = quote! {
        #[::core::prelude::v1::test]
        #(#attrs)*
        #vis #sig {
            ::futures_test::__private::block_on(async move #body)
        }
    };

    gen.into()
}
