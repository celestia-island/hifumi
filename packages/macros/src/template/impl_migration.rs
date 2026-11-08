use anyhow::{anyhow, Result};
use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::{Ident, Type};

use crate::{
    tools::{MigrationComment, MigrationField},
    utils::generate_ident,
};

use super::old_version_structs::infer_older_version_struct;

fn generate_older_version_impl(
    old_struct_fields: BTreeMap<Ident, Type>,
    convert_rules: Vec<MigrationField>,
) -> Result<BTreeMap<Ident, TokenStream>> {
    let mut struct_fields = old_struct_fields
        .keys()
        .map(|key| {
            (
                key.clone(),
                quote! {
                    #key: __old.#key.to_owned()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    for rule in convert_rules.iter() {
        match rule {
            MigrationField::Add { value, converter } => {
                let (key, ty) = value;

                match converter {
                    Some(converter) => {
                        struct_fields.insert(
                            key.clone(),
                            quote! {
                                #key: { #converter }
                            },
                        );
                    }
                    None => {
                        struct_fields.insert(
                            key.clone(),
                            quote! {
                                #key: #ty::default()
                            },
                        );
                    }
                }
            }
            MigrationField::Copy {
                source,
                target,
                converter,
            } => {
                let (target_ident, target_ty) = target;

                match converter {
                    Some(converter) => {
                        let params = source
                            .iter()
                            .map(|(key, ty)| quote! { #key: #ty })
                            .collect::<Vec<_>>();
                        let converter = quote! {
                            #[allow(unused_variables)]
                            fn _converter(#(#params),*) -> #target_ty {
                                #converter
                            }
                        };
                        let args = source
                            .iter()
                            .map(|(key, _)| quote! { __old.#key.clone() })
                            .collect::<Vec<_>>();

                        struct_fields.insert(
                            target_ident.clone(),
                            quote! {
                                #target_ident: {
                                    #converter
                                    _converter(#(#args),*)
                                }
                            },
                        );
                    }
                    None => {
                        if source.len() == 1 {
                            let (source_ident, source_ty) = source.iter().next().unwrap();

                            if source_ty.to_token_stream().to_string()
                                == target_ty.to_token_stream().to_string()
                            {
                                struct_fields.insert(
                                    target_ident.clone(),
                                    quote! {
                                        #target_ident: __old.#source_ident.clone()
                                    },
                                );
                            } else {
                                struct_fields.insert(
                                    target_ident.clone(),
                                    quote! {
                                        #target_ident: __old.#source_ident.clone().into()
                                    },
                                );
                            }
                        } else {
                            return Err(anyhow!(
                                "Must provide a converter while copying field with multiple source"
                            ));
                        }
                    }
                }
            }
            MigrationField::Remove { value, .. } => {
                let (ident, _) = value;
                struct_fields.remove(ident);
            }
            MigrationField::Rename {
                source,
                target,
                converter,
                ..
            } => {
                for (ident, _) in source.iter() {
                    struct_fields.remove(ident);
                }

                let (target_ident, target_ty) = target;

                match converter {
                    Some(converter) => {
                        let params = source
                            .iter()
                            .map(|(key, ty)| quote! { #key: #ty })
                            .collect::<Vec<_>>();
                        let converter = quote! {
                            #[allow(unused_variables)]
                            fn _converter(#(#params),*) -> #target_ty {
                                #converter
                            }
                        };
                        let args = source
                            .iter()
                            .map(|(key, _)| quote! { __old.#key.clone() })
                            .collect::<Vec<_>>();

                        struct_fields.insert(
                            target_ident.clone(),
                            quote! {
                                #target_ident: {
                                    #converter
                                    _converter(#(#args),*)
                                }
                            },
                        );
                    }
                    None => {
                        if source.len() == 1 {
                            let (source_ident, source_ty) = source.iter().next().unwrap();

                            if source_ty.to_token_stream().to_string()
                                == target_ty.to_token_stream().to_string()
                            {
                                struct_fields.insert(
                                    target_ident.clone(),
                                    quote! {
                                        #target_ident: __old.#source_ident
                                    },
                                );
                            } else {
                                struct_fields.insert(
                                    target_ident.clone(),
                                    quote! {
                                        #target_ident: __old.#source_ident.into()
                                    },
                                );
                            }
                        } else {
                            return Err(anyhow!("Must provide a converter while renaming field"));
                        }
                    }
                }
            }
        }
    }

    Ok(struct_fields.into_iter().collect())
}

pub(crate) fn generate_impl_froms(
    ident: Ident,
    final_version: String,
    final_struct_fields: BTreeMap<Ident, Type>,
    versions: Vec<MigrationComment>,
) -> Result<TokenStream> {
    let mut temp_struct_fields = final_struct_fields.to_owned();
    let mut temp_version = final_version.clone();
    let mut impl_froms = vec![];
    // Audit hifumi-R1-1: same cycle guard as the version() macro walk —
    // a DSL self-loop/2-cycle would loop here forever.
    let mut visited = std::collections::HashSet::new();

    while let Some(item) = versions.iter().find(|item| item.to.value() == temp_version) {
        let to = item.to.value();
        if !visited.insert(to.clone()) {
            return Err(anyhow::anyhow!(
                "cycle in migration chain: version {to:?} is revisited; \
                 each migration must move strictly backwards"
            ));
        }
        temp_version = item.from.value();
        // 降级（新 → 旧）需要该步「新」侧的字段表——在 infer 成旧侧之前留存
        let newer_struct_fields = temp_struct_fields.clone();
        temp_struct_fields =
            infer_older_version_struct(temp_struct_fields.clone(), item.changes.clone())?;

        let from_ident = generate_ident(&ident, item.from.value())?;
        let to_ident = if item.to.value() == final_version {
            ident.clone()
        } else {
            generate_ident(&ident, item.to.value())?
        };

        let temp_struct_impl =
            generate_older_version_impl(temp_struct_fields.clone(), item.changes.clone())?;
        let temp_struct_impl_nearly = temp_struct_impl.values();

        impl_froms.push(quote! {
            impl From<#from_ident> for #to_ident {
                fn from(__old: #from_ident) -> Self {
                    Self {
                        #(#temp_struct_impl_nearly),*
                    }
                }
            }
        });

        if item.to.value() != final_version {
            impl_froms.push(quote! {
                impl From<#from_ident> for #ident {
                    fn from(__old: #from_ident) -> Self {
                        #to_ident::from(__old).into()
                    }
                }
            });
        }

        // 反向：该步每条规则都免费或带 back 时生成 From<新> for 旧。
        // 链首步（to == final）的新侧就是最终结构体本身，to_ident 已处理该替换。
        if step_missing_back(&item.changes, &newer_struct_fields).is_none() {
            let reverse_impl =
                generate_reverse_impl(&newer_struct_fields, &item.changes, &from_ident)?;
            impl_froms.push(quote! {
                impl From<#to_ident> for #from_ident {
                    fn from(__new: #to_ident) -> Self {
                        #reverse_impl
                    }
                }
            });
        }
    }

    let final_ident = generate_ident(&ident, &final_version)?;
    let final_fields = final_struct_fields
        .keys()
        .map(|ident| {
            quote! {
                #ident: __old.#ident.to_owned()
            }
        })
        .collect::<Vec<_>>();

    impl_froms.push(quote! {
        impl From<#final_ident> for #ident {
            fn from(__old: #final_ident) -> Self {
                Self {
                    #(#final_fields),*
                }
            }
        }
    });
    impl_froms.push(quote! {
        impl From<#ident> for #final_ident {
            fn from(__old: #ident) -> Self {
                Self {
                    #(#final_fields),*
                }
            }
        }
    });

    Ok(quote! {
        #(#impl_froms)*
    })
}

/// 反向（新 → 旧）走一步时，第一条缺 `back` 而又不免费可逆的规则对应的字段。
/// 返回 None = 该步可逆。免费可逆 = Add/Copy（丢字段）或「单源、无前向变换、
/// 同类型」的纯改名；其余 Remove/Rename 必须显式给 `back`。
/// 判断一步迁移是否可逆，返回第一个缺反向转换的字段名。
///
/// `newer_fields` 是该步「新」侧结构体的字段表。`available` 集合**镜像**
/// [`generate_reverse_impl`] 里 struct_fields 的键演进（按规则序消费目标 /
/// 回填源），因此判定与实际代码生成逐字段一致：
/// - 纯改名只有在改名目标届时仍可读（没有被先前的规则消费掉）时才免费可逆；
/// - Copy 的每个源字段届时必须仍可读（被先前规则目标遮蔽时旧值无法重建，
///   Copy 又没有 `back` 语法 → 该步不可逆）。
pub(crate) fn step_missing_back(
    convert_rules: &[MigrationField],
    newer_fields: &BTreeMap<Ident, Type>,
) -> Option<String> {
    let mut available: std::collections::BTreeSet<Ident> = newer_fields.keys().cloned().collect();

    for rule in convert_rules {
        match rule {
            MigrationField::Add { value, .. } => {
                available.remove(&value.0);
            }
            MigrationField::Copy { source, target, .. } => {
                for (src, _) in source {
                    if !available.contains(src) {
                        return Some(src.to_string());
                    }
                }
                available.remove(&target.0);
            }
            MigrationField::Remove { value, back } => {
                if back.is_none() {
                    return Some(value.0.to_string());
                }
                available.insert(value.0.clone());
            }
            MigrationField::Rename {
                source,
                target,
                converter,
                back,
            } => {
                if back.is_none() {
                    let pure_rename = converter.is_none()
                        && source.len() == 1
                        && available.contains(&target.0)
                        && {
                            let (_, src_ty) = source.iter().next().unwrap();
                            let (_, tgt_ty) = target;
                            src_ty.to_token_stream().to_string()
                                == tgt_ty.to_token_stream().to_string()
                        };
                    if !pure_rename {
                        let names = source
                            .iter()
                            .map(|(ident, _)| ident.to_string())
                            .collect::<Vec<_>>()
                            .join(", ");
                        return Some(names);
                    }
                    // 纯改名反向 = 从目标克隆回源
                    let (src_key, _) = source.iter().next().unwrap();
                    let (tgt_key, _) = target;
                    available.remove(tgt_key);
                    available.insert(src_key.clone());
                } else {
                    let (tgt_key, _) = target;
                    available.remove(tgt_key);
                    for (src_key, _) in source {
                        available.insert(src_key.clone());
                    }
                }
            }
        }
    }
    None
}

/// 生成「新 → 旧」一步的表达式：一个 block，先对每条带 `back` 的规则求值一次
/// （back 作用域 = 新结构体全部字段，作为自由标识符传入 helper），再构造旧结构体。
/// 调用方需先用 [`step_missing_back`] 确认该步可逆。
fn generate_reverse_impl(
    newer_struct_fields: &BTreeMap<Ident, Type>,
    convert_rules: &[MigrationField],
    older_ident: &Ident,
) -> Result<TokenStream> {
    // 旧结构字段初始化：基底 = 新结构字段 1:1（Add/Copy 的差异字段随后剔除）
    let mut struct_fields = newer_struct_fields
        .keys()
        .map(|key| {
            (
                key.clone(),
                quote! {
                    #key: __new.#key.to_owned()
                },
            )
        })
        .collect::<BTreeMap<Ident, TokenStream>>();
    let mut locals = Vec::new();

    for (rule_index, rule) in convert_rules.iter().enumerate() {
        match rule {
            MigrationField::Add { value, .. } => {
                struct_fields.remove(&value.0);
            }
            MigrationField::Copy { target, .. } => {
                struct_fields.remove(&target.0);
            }
            MigrationField::Remove { value, back } => {
                let (key, ty) = value;
                let back = back.as_ref().ok_or_else(|| {
                    anyhow!("remove rule for `{}` is missing a back converter", key)
                })?;
                let local = format_ident!("__hifumi_back_{}", rule_index);
                let params = newer_struct_fields.iter().map(|(k, t)| quote! { #k: #t });
                let args = newer_struct_fields
                    .keys()
                    .map(|k| quote! { __new.#k.clone() });
                locals.push(quote! {
                    #[allow(unused_variables)]
                    let #local = {
                        #[allow(unused_variables)]
                        fn _back(#(#params),*) -> #ty { #back }
                        _back(#(#args),*)
                    };
                });
                struct_fields.insert(key.clone(), quote! { #key: #local });
            }
            MigrationField::Rename {
                source,
                target,
                back,
                ..
            } => {
                struct_fields.remove(&target.0);
                if let Some(back) = back {
                    let params = newer_struct_fields.iter().map(|(k, t)| quote! { #k: #t });
                    let args = newer_struct_fields
                        .keys()
                        .map(|k| quote! { __new.#k.clone() });
                    if source.len() == 1 {
                        let (key, ty) = &source[0];
                        let local = format_ident!("__hifumi_back_{}", rule_index);
                        locals.push(quote! {
                            #[allow(unused_variables)]
                            let #local = {
                                #[allow(unused_variables)]
                                fn _back(#(#params),*) -> #ty { #back }
                                _back(#(#args),*)
                            };
                        });
                        struct_fields.insert(key.clone(), quote! { #key: #local });
                    } else {
                        // 多源：back 求值为元组，解构后逐字段搬入
                        let tys = source.iter().map(|(_, t)| quote! { #t });
                        let binds = source
                            .iter()
                            .enumerate()
                            .map(|(i, _)| format_ident!("__hifumi_back_{}_{}", rule_index, i))
                            .collect::<Vec<_>>();
                        locals.push(quote! {
                            #[allow(unused_variables)]
                            let (#(#binds),*) = {
                                #[allow(unused_variables)]
                                fn _back(#(#params),*) -> (#(#tys),*) { #back }
                                _back(#(#args),*)
                            };
                        });
                        for ((key, _), bind) in source.iter().zip(binds) {
                            struct_fields.insert(key.clone(), quote! { #key: #bind });
                        }
                    }
                } else {
                    // 纯改名（单源、无变换、同类型）：反向 = 从目标字段克隆回源
                    let (src_key, _) = source.iter().next().unwrap();
                    let (tgt_key, _) = target;
                    struct_fields.insert(
                        src_key.clone(),
                        quote! { #src_key: __new.#tgt_key.to_owned() },
                    );
                }
            }
        }
    }

    let inits = struct_fields.into_values();
    Ok(quote! {
        {
            #(#locals)*
            #older_ident {
                #(#inits),*
            }
        }
    })
}
