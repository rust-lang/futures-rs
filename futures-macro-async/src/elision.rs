use proc_macro2::Span;
use syn::*;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::fold::Fold;

pub fn unelide_lifetimes(generics: &mut Punctuated<GenericParam, Comma>, args: Vec<FnArg>)
    -> Vec<FnArg>
{
    let mut folder = UnelideLifetimes::new(generics);
    args.into_iter().map(|arg| folder.fold_fn_arg(arg)).collect()
}

struct UnelideLifetimes<'a> {
    generics: &'a mut Punctuated<GenericParam, Comma>,
    lifetime_index: usize,
    lifetime_name: String,
    count: u32,
}

impl<'a> UnelideLifetimes<'a> {
    fn new(generics: &'a mut Punctuated<GenericParam, Comma>) -> UnelideLifetimes<'a> {
        let lifetime_index = lifetime_index(generics);
        let lifetime_name = lifetime_name(generics);
        UnelideLifetimes {
            generics,
            lifetime_index,
            lifetime_name,
            count: 0
        }

    }

    // Constitute a new lifetime
    fn new_lifetime(&mut self) -> Lifetime {
        let lifetime_name = format!("{}{}", self.lifetime_name, self.count);
        let lifetime = Lifetime::new(&lifetime_name, Span::call_site());

        let idx = self.lifetime_index + self.count as usize;
        self.generics.insert(idx, GenericParam::Lifetime(LifetimeDef::new(lifetime.clone())));
        self.count += 1;

        lifetime
    }

    // Take an Option<Lifetime> and guarantee its an unelided lifetime
    fn expand_lifetime(&mut self, lifetime: Option<Lifetime>) -> Lifetime {
        match lifetime {
            Some(l) => self.fold_lifetime(l),
            None    => self.new_lifetime(),
        }
    }
}

impl<'a> Fold for UnelideLifetimes<'a> {
    // Handling self arguments
    fn fold_arg_self_ref(&mut self, arg: ArgSelfRef) -> ArgSelfRef {
        let ArgSelfRef { and_token, lifetime, mutability, self_token } = arg;
        let lifetime = Some(self.expand_lifetime(lifetime));
        ArgSelfRef { and_token, lifetime, mutability, self_token }
    }

    // If the lifetime is `'_`, replace it with a new unelided lifetime
    fn fold_lifetime(&mut self, lifetime: Lifetime) -> Lifetime {
        if lifetime.to_string() == "'_" { self.new_lifetime() }
        else { lifetime }
    }

    // If the reference's lifetime is elided, replace it with a new unelided lifetime
    fn fold_type_reference(&mut self, ty_ref: TypeReference) -> TypeReference {
        let TypeReference { and_token, lifetime, mutability, elem } = ty_ref;
        let lifetime = Some(self.expand_lifetime(lifetime));
        let elem = Box::new(self.fold_type(*elem));
        TypeReference { and_token, lifetime, mutability, elem }
    }
}

fn lifetime_index(generics: &Punctuated<GenericParam, Comma>) -> usize {
    generics.iter()
        .take_while(|param| if let GenericParam::Lifetime(_) = param { true } else { false })
        .count()
}

// Determine the prefix for all lifetime names. Ensure it doesn't
// overlap with any existing lifetime names.
fn lifetime_name(generics: &Punctuated<GenericParam, Comma>) -> String {
    let mut lifetime_name = String::from("'_async");
    let existing_lifetimes: Vec<String> = generics.iter().filter_map(|param| {
        if let GenericParam::Lifetime(LifetimeDef { lifetime, .. }) = param { Some(lifetime.to_string()) }
        else { None }
    }).collect();
    while existing_lifetimes.iter().any(|name| name.starts_with(&lifetime_name)) {
        lifetime_name.push('_');
    }
    lifetime_name
}
