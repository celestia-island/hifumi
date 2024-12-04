use anyhow::Result;
use proc_macro2::TokenStream;
use quote::quote;

use crate::tools::{DeriveVersion, Migration};

pub(crate) fn generate_current_version_struct(
    attr: DeriveVersion,
    input: Migration,
) -> Result<TokenStream> {
    let Migration {
        extra_macros,
        struct_data,
        ..
    } = input;

    let extra_macros = extra_macros.iter().map(|(key, value)| {
        quote! {
            #[#key #value]
        }
    });

    Ok(quote! {
        #(#extra_macros)*
        #struct_data
    })
}
