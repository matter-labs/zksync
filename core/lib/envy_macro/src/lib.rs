use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(EnvLoad)]
pub fn env_load(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let gen = quote! {
        impl EnvLoad for #name {
            fn from_env() -> Self {
               envy::prefixed($prefix)
                .from_env()
                .unwrap_or_else(|err| panic!("Cannot load config <{}>: {}", stringify!(#name), err))
            }
        }
    };
    gen.into()
}
