use proc_macro::TokenStream;
use quote::quote;

#[allow(clippy::cmp_owned)] // False positive, `syn::Ident` cannot be dereferenced.
fn is_arg_storage_processor(arg: Option<&syn::FnArg>) -> bool {
    if let Some(syn::FnArg::Typed(arg)) = arg {
        // For now, we just assume that people writing tests know what they're doing and if method has
        // exactly one argument and it's named correctly, it is supposed to be the right one.
        if let syn::Pat::Ident(ident) = arg.pat.as_ref() {
            if ident.ident.to_string() == "storage" {
                return true;
            }
        }
    }
    false
}

fn parse_knobs(mut input: syn::ItemFn) -> Result<TokenStream, syn::Error> {
    let sig = &mut input.sig;
    let body = &input.block;
    let attrs = &input.attrs;
    let vis = input.vis;

    if sig.asyncness.is_none() {
        let msg = "the async keyword is missing from the function declaration";
        return Err(syn::Error::new_spanned(sig.fn_token, msg));
    }

    sig.asyncness = None;

    if sig.inputs.len() != 1 || !is_arg_storage_processor(sig.inputs.first()) {
        let msg = "the DB test function must take `mut storage: zksync_storage::StorageProcessor<'_>` as a single argument";
        return Err(syn::Error::new_spanned(&sig.inputs, msg));
    }

    // Remove argument, as the test function must not have one.
    sig.inputs.pop();

    let rt = quote! { tokio::runtime::Builder::new().basic_scheduler() };

    let header = quote! {
        #[::core::prelude::v1::test]
        #[cfg_attr(not(feature = "db_test"), ignore)]
    };

    let result = quote! {
        #header
        #(#attrs)*
        #vis #sig {
            #rt
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let mut connection = StorageProcessor::establish_connection().await.unwrap();
                    // `storage` is a transaction, which will be dropped, and thus not committed.
                    let mut storage = connection.start_transaction().await.unwrap();
                    #body
                })
        }
    };

    Ok(result.into())
}

#[proc_macro_attribute]
pub fn test(_args: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);

    for attr in &input.attrs {
        if attr.path.is_ident("test") {
            let msg = "second test attribute is supplied";
            return syn::Error::new_spanned(&attr, msg)
                .to_compile_error()
                .into();
        }
    }

    parse_knobs(input).unwrap_or_else(|e| e.to_compile_error().into())
}
