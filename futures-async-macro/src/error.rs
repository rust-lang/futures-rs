// Make error messages a little more readable than `panic!`.
macro_rules! error {
    ($msg:expr) => {
        return proc_macro::TokenStream::from(
            syn::Error::new(proc_macro2::Span::call_site(), $msg).to_compile_error(),
        )
    };
    ($span:expr, $msg:expr) => {
        return proc_macro::TokenStream::from(
            syn::Error::new_spanned($span, $msg).to_compile_error(),
        )
    };
}

// TODO: Should we give another name?
// `assert!` that call `error!` instead of `panic!`.
macro_rules! assert_ {
    ($e:expr, $msg:expr) => {
        if !$e {
            error!($msg)
        }
    };
}

pub(super) fn expr_compile_error(e: &syn::Error) -> syn::Expr {
    syn::parse2(e.to_compile_error()).unwrap()
}

// Long error messages and error messages that are called multiple times

macro_rules! args_is_not_empty {
    ($name:expr) => {
        concat!("attribute must be of the form `#[", $name, "]`")
    };
}

macro_rules! outside_of_async_error {
    ($tokens:expr, $name:expr) => {
        $crate::error::expr_compile_error(&syn::Error::new_spanned(
            $tokens,
            concat!(
                $name,
                " cannot be allowed outside of \
                 async closures, blocks, functions, async_stream blocks, and functions."
            ),
        ))
    };
}
