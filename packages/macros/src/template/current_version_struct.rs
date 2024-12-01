use anyhow::Result;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::tools::DeriveVersion;

pub(crate) fn generate_current_version_struct(
    name: Ident,
    input: DeriveVersion,
) -> Result<TokenStream> {
    dbg!(name, input);

    Ok(quote! {})
}
