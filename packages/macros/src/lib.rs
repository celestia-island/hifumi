use anyhow::{anyhow, Result};
use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

mod template;
mod tools;
mod utils;

use template::{generate_current_version_struct, generate_impl_from};
use tools::{DeriveVersion, Migration};
use utils::generate_ident;

#[proc_macro_attribute]
pub fn version(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attr: DeriveVersion = parse_macro_input!(attr);
    let input: Migration = parse_macro_input!(input);

    let current_version_struct = generate_current_version_struct(attr.clone(), input.clone())
        .expect("Failed to generate current version struct");

    // Confirm the order of the versions
    let versions = input
        .versions
        .iter()
        .map(|(from, (to, rules))| (from.value(), to.value(), rules.to_owned()))
        .collect::<Vec<_>>();
    let versions = versions
        .iter()
        .map(|(from, to, convert_rules)| {
            Ok(generate_impl_from(
                input
                    .struct_data
                    .fields
                    .iter()
                    .map(|field| match field {
                        syn::Field {
                            ident: Some(ident),
                            ty,
                            ..
                        } => Ok((ident.clone(), ty.clone())),
                        _ => Err(anyhow!("Failed to get field ident")),
                    })
                    .collect::<Vec<Result<_>>>()
                    .into_iter()
                    .collect::<Result<Vec<_>>>()?,
                generate_ident(input.struct_data.ident.clone(), from)?,
                generate_ident(input.struct_data.ident.clone(), to)?,
                convert_rules.changes.clone(),
            )?)
        })
        .collect::<Vec<Result<_>>>()
        .into_iter()
        .collect::<Result<Vec<_>>>()
        .expect("Failed to generate impl from");

    quote! {
        #current_version_struct
        #(#versions)*
    }
    .into()
}
