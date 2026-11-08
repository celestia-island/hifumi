use std::collections::BTreeMap;

use anyhow::Result;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Ident, LitBool, LitStr, Type};

use crate::{
    tools::{DeriveVersion, Migration, MigrationField},
    utils::generate_ident,
};

use self::old_version_structs::generate_old_versions;

use super::impl_migration::chain_reversible;
use super::old_version_structs;

pub(crate) fn generate_current_version_struct(
    attr: DeriveVersion,
    input: Migration,
    ident: Ident,
    final_version: String,
    final_struct_fields: BTreeMap<Ident, Type>,
    versions: Vec<(String, Vec<MigrationField>, String)>,
) -> Result<TokenStream> {
    let Migration {
        extra_macros,
        struct_data,
        ..
    } = input;

    let old_version_structs = generate_old_versions(
        final_version.clone(),
        final_struct_fields.clone(),
        versions.clone(),
    )?;
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
        #[serde(tag = "$version")]
        enum #old_version_structs_enum_name {
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

    // ① 前向兼容旁路：`try_from_value` 先按已知版本枚举走正常
    // 反序列化；失败（未知/缺失 `$version`，即更新版本的数据）时回退
    // 为宽容读者——按当前版本表示解析整张 map，未知字段静默丢弃。
    // 已知旧版本照常走迁移链，不影响。
    let impl_try_from_value = quote! {
        #[automatically_derived]
        impl ::hifumi::TryFromValue for #ident {
            /// 从 JSON 值解析本类型：已知版本走迁移链；未知或缺失
            /// `$version` 的数据按宽容读者语义取当前版本可识别的字段。
            fn try_from_value(
                value: &::hifumi::serde_json::Value,
            ) -> ::core::result::Result<Self, ::hifumi::serde_json::Error> {
                match ::hifumi::serde_json::from_value::<#old_version_structs_enum_name>(
                    value.clone(),
                ) {
                    Ok(parsed) => match parsed {
                        #( #impl_deserialize_match_list, )*
                    },
                    Err(_) => {
                        ::hifumi::serde_json::from_value::<#impl_serialize_final_version_ident>(
                            value.clone(),
                        )
                        .map(|v| v.into())
                    }
                }
            }
        }
    };

    // 未开 forward 旗标：直接按版本枚举反序列化（零额外开销，
    // 未知版本 = 编译期可预测的 unknown variant 错误，行为与历史一致）。
    let impl_deserialize = if attr.forward {
        quote! {
            impl<'de> ::serde::Deserialize<'de> for #ident {
                fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
                where
                    D: ::serde::Deserializer<'de>,
                {
                    let value = ::hifumi::serde_json::Value::deserialize(deserializer)?;
                    <Self as ::hifumi::TryFromValue>::try_from_value(&value)
                        .map_err(::serde::de::Error::custom)
                }
            }
        }
    } else {
        quote! {
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
        }
    };

    // ④ 中央注册表元数据：类型名、全版本链（旧 → 新）、整链可逆性与
    // forward 旗标。`hifumi::registry!` 据此做编译期断言并生成 `spec()`。
    let meta_version_lits = versions
        .iter()
        .map(|(from, _, _)| LitStr::new(from, Span::call_site()))
        .rev()
        .chain(std::iter::once(LitStr::new(
            &final_version,
            Span::call_site(),
        )))
        .collect::<Vec<_>>();
    let meta_reversible = LitBool::new(
        chain_reversible(&final_version, &final_struct_fields, &versions),
        Span::call_site(),
    );
    let meta_forward = LitBool::new(attr.forward, Span::call_site());
    let meta_const = quote! {
        #[automatically_derived]
        impl #ident {
            #[doc(hidden)]
            pub const __HIFUMI_META: ::hifumi::TypeMeta = ::hifumi::TypeMeta {
                type_name: ::core::stringify!(#ident),
                versions: &[#(#meta_version_lits),*],
                reversible: #meta_reversible,
                forward: #meta_forward,
            };
        }
    };

    Ok(quote! {
        #(#extra_macros)*
        #struct_data

        #impl_serialize
        #impl_deserialize
        #impl_try_from_value
        #meta_const
        #old_version_structs_enum
    })
}
