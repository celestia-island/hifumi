use std::collections::HashMap;

use anyhow::Result;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Ident, LitStr, Type};

use crate::{
    tools::{DeriveVersion, Migration, MigrationField},
    utils::generate_ident,
};

use self::old_version_structs::generate_old_versions;

use super::old_version_structs;

pub(crate) fn generate_current_version_struct(
    attr: DeriveVersion,
    input: Migration,
    ident: Ident,
    final_version: String,
    final_struct_fields: HashMap<Ident, Type>,
    versions: Vec<(String, Vec<MigrationField>, String)>,
) -> Result<TokenStream> {
    let Migration {
        extra_macros,
        struct_data,
        ..
    } = input;

    let old_version_structs =
        generate_old_versions(final_version.clone(), final_struct_fields, versions)?;
    let old_version_structs_enum = old_version_structs
        .clone()
        .iter()
        .map(|(version, _)| {
            Ok((
                generate_ident(&ident, version)?,
                LitStr::new(version, Span::call_site()),
                generate_ident(&ident, version)?,
            ))
        })
        .collect::<Vec<Result<_>>>()
        .into_iter()
        .collect::<Result<Vec<_>>>()?;
    let old_version_structs_enum = old_version_structs_enum
        .iter()
        .map(|(enum_name, enum_rename_litstr, enum_ty)| {
            quote! {
                #[serde(rename = #enum_rename_litstr)]
                #enum_name(#enum_ty),
            }
        })
        .collect::<Vec<TokenStream>>();
    let old_version_structs_enum_name = generate_ident(&ident, "#outer")?;
    let old_version_structs_enum = quote! {
        #[doc(hidden)]
        #[allow(non_camel_case_types, unused_variables, dead_code)]
        #[derive(Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(tag = "$version")]  // TODO: Read from attribute
        pub enum #old_version_structs_enum_name {
            #(#old_version_structs_enum)*
        }
    };

    let extra_macros = extra_macros.iter().map(|(key, value)| {
        quote! {
            #[#key #value]
        }
    });

    // Generate serialize implementation

    let impl_serialize_final_version_ident = generate_ident(&ident, &final_version)?;
    let impl_serialize = quote! {
        impl ::serde::Serialize for #ident {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                #old_version_structs_enum_name::#impl_serialize_final_version_ident(
                    #impl_serialize_final_version_ident::from(self.to_owned())
                ).serialize(serializer)
            }
        }
    };

    // Generate deserialize implementation
    let impl_deserialize_match_list = old_version_structs
        .iter()
        .map(|(version, _)| {
            let struct_name = generate_ident(&ident, version)?;
            Ok(quote! {
                #old_version_structs_enum_name::#struct_name(val) => Ok(val.into())
            })
        })
        .collect::<Vec<Result<TokenStream>>>()
        .into_iter()
        .collect::<Result<Vec<TokenStream>>>()?;
    let impl_deserialize = quote! {
        impl<'de> ::serde::Deserialize<'de> for #ident {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                let value = #old_version_structs_enum_name::deserialize(deserializer)?;
                match value {
                    #( #impl_deserialize_match_list, )*
                }
            }
        }
    };

    Ok(quote! {
        #(#extra_macros)*
        #struct_data

        #impl_serialize
        #impl_deserialize
        #old_version_structs_enum
    })
}
