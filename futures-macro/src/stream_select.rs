use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Expr, Index, parse::Parser, punctuated::Punctuated, Token};

/// The `stream_select!` macro.
pub(crate) fn stream_select(input: TokenStream) -> TokenStream {
    let args = Punctuated::<Expr, Token![,]>::parse_terminated.parse(input).expect("macro expects a comma separated list of expressions");
    let generic_idents = (0..args.len()).map(|i| format_ident!("_{}", i)).collect::<Vec<_>>();
    let field_idents = (0..args.len()).map(|i| format_ident!("__{}", i)).collect::<Vec<_>>();
    let field_indices = (0..args.len()).map(Index::from).collect::<Vec<_>>();

    TokenStream::from(quote! {
        {
            struct StreamSelect<#(#generic_idents),*> (#(#generic_idents),*);

            enum StreamFutures<#(#generic_idents),*> {
                #(
                    #generic_idents(#generic_idents)
                ),*
            }

            impl<OUTPUT, #(#generic_idents),*> __futures_crate::future::Future for StreamFutures<#(#generic_idents),*>
            where #(#generic_idents: __futures_crate::future::Future<Output=OUTPUT> + __futures_crate::future::FusedFuture + ::std::marker::Unpin,)*
            {
                type Output = OUTPUT;

                fn poll(mut self: ::std::pin::Pin<&mut Self>, cx: &mut __futures_crate::task::Context<'_>) -> __futures_crate::task::Poll<Self::Output> {
                    match self.get_mut() {
                        #(
                            Self::#generic_idents(#generic_idents) => ::std::pin::Pin::new(#generic_idents).poll(cx)
                        ),*
                    }
                }
            }

            impl<OUTPUT, #(#generic_idents),*> __futures_crate::future::FusedFuture for StreamFutures<#(#generic_idents),*>
            where #(#generic_idents: __futures_crate::future::Future<Output=OUTPUT> + __futures_crate::future::FusedFuture + ::std::marker::Unpin,)*
            {
                fn is_terminated(&self) -> bool {
                    match self {
                        #(
                            Self::#generic_idents(#generic_idents) => __futures_crate::future::FusedFuture::is_terminated(#generic_idents)
                        ),*
                    }
                }
            }

            impl<ITEM, #(#generic_idents),*> __futures_crate::stream::Stream for StreamSelect<#(#generic_idents),*>
            where #(#generic_idents: __futures_crate::stream::Stream<Item=ITEM> + __futures_crate::stream::FusedStream + ::std::marker::Unpin,)*
            {
                type Item = ITEM;

                fn poll_next(mut self: ::std::pin::Pin<&mut Self>, cx: &mut __futures_crate::task::Context<'_>) -> __futures_crate::task::Poll<Option<Self::Item>> {
                    let Self(#(ref mut #field_idents),*) = self.get_mut();
                    let mut future_array = [#(StreamFutures::#generic_idents(#field_idents.next())),*];
                    __futures_crate::async_await::shuffle(&mut future_array);
                    let mut any_pending = false;
                    for f in &mut future_array {
                        if __futures_crate::future::FusedFuture::is_terminated(f) {
                            continue;
                        } else {
                            match __futures_crate::future::Future::poll(::std::pin::Pin::new(f), cx) {
                                r @ __futures_crate::task::Poll::Ready(Some(_)) => {
                                    return r;
                                },
                                __futures_crate::task::Poll::Pending => {
                                    any_pending = true;
                                },
                                __futures_crate::task::Poll::Ready(None) => {},
                            }
                        }
                    }
                    if any_pending {
                        __futures_crate::task::Poll::Pending
                    } else {
                        __futures_crate::task::Poll::Ready(None)
                    }
                }

                fn size_hint(&self) -> (usize, Option<usize>) {
                    let mut s = (0, Some(0));
                    #(
                        {
                            let new_hint = self.#field_indices.size_hint();
                            s.0 += new_hint.0;
                            // We can change this out for `.zip` when the MSRV is 1.46.0 or higher.
                            s.1 = s.1.and_then(|a| new_hint.1.map(|b| a + b));
                        }
                    )*
                    s
                }
            }

            StreamSelect(#args)
        }
    })
}
