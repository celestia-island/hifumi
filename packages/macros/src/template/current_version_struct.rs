use anyhow::Result;
use proc_macro2::TokenStream;
use quote::quote;

use crate::tools::{DeriveVersion, Migration};

pub(crate) fn generate_current_version_struct(
    attr: DeriveVersion,
    input: Migration,
) -> Result<TokenStream> {
    dbg!(attr, input);

    Ok(quote! {})
}
