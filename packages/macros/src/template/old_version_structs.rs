use anyhow::Result;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::{Ident, Type};

use crate::{tools::MigrationField, utils::generate_ident};

pub(crate) fn infer_older_version_struct(
    newer_struct_fields: HashMap<Ident, Type>,
    convert_rules: Vec<MigrationField>,
) -> Result<HashMap<Ident, Type>> {
    let mut struct_fields = newer_struct_fields.to_owned();

    for rule in convert_rules.iter() {
        match rule {
            MigrationField::AddField { value, .. } => {
                let (key, _) = value;
                struct_fields.remove(&key);
            }
            MigrationField::CopyField { source, target, .. } => {
                let (target_ident, _) = target;
                struct_fields.remove(&target_ident);

                for (ident, ty) in source.iter() {
                    struct_fields.insert(ident.clone(), Type::Path(ty.clone()));
                }
            }
            MigrationField::RemoveField { value } => {
                let (ident, ty) = value;
                struct_fields.insert(ident.clone(), Type::Path(ty.clone()));
            }
            MigrationField::RenameField { source, target, .. } => {
                let (target_ident, _) = target;
                struct_fields.remove(&target_ident);

                for (ident, ty) in source.iter() {
                    struct_fields.insert(ident.clone(), Type::Path(ty.clone()));
                }
            }
        }
    }

    Ok(struct_fields.into_iter().collect())
}

pub(crate) fn generate_old_versions(
    final_version: String,
    final_struct_fields: HashMap<Ident, Type>,
    versions: Vec<(String, Vec<MigrationField>)>,
) -> Result<Vec<(String, HashMap<Ident, Type>)>> {
    let mut temp_struct_fields = final_struct_fields.to_owned();
    let mut old_version_structs = vec![(final_version, temp_struct_fields.clone())];

    for (version, convert_rules) in versions.iter() {
        temp_struct_fields =
            infer_older_version_struct(temp_struct_fields.clone(), convert_rules.to_owned())?;
        old_version_structs.push((version.clone(), temp_struct_fields.clone()));
    }

    Ok(old_version_structs)
}

pub(crate) fn generate_old_version_structs(
    ident: Ident,
    final_version: String,
    final_struct_fields: HashMap<Ident, Type>,
    extra_macros: Vec<TokenStream>,
    versions: Vec<(String, Vec<MigrationField>)>,
) -> Result<TokenStream> {
    let old_version_structs =
        generate_old_versions(final_version.clone(), final_struct_fields, versions)?;

    let old_version_structs = old_version_structs
        .iter()
        .map(|(version, fields)| {
            if version == &final_version {
                return Ok(quote! {});
            }
            let struct_name = generate_ident(&ident, version)?;
            let fields = fields.iter().map(|(ident, ty)| {
                quote! {
                    #ident: #ty,
                }
            });

            Ok(quote! {
                #[doc(hidden)]
                #[allow(non_camel_case_types, unused_variables, dead_code)]
                #(#extra_macros)*
                #[derive(::serde::Serialize, ::serde::Deserialize)]
                pub struct #struct_name {
                    #(#fields)*
                }
            })
        })
        .collect::<Vec<Result<TokenStream>>>()
        .into_iter()
        .collect::<Result<Vec<TokenStream>>>()?;

    Ok(quote! {
        #(#old_version_structs)*
    })
}
