use anyhow::Result;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use std::collections::BTreeMap;
use syn::{Attribute, Ident, Type};

use crate::{tools::MigrationField, utils::generate_ident};

/// 字段名 → 逐字段透传属性（见 [`crate::tools::Migration::field_passthrough_attrs`]）。
/// 单字段没有白名单属性时，`get` 落空即不生成任何 attribute。
pub(crate) type FieldPassthrough = BTreeMap<Ident, Vec<Attribute>>;

/// 当前版本的字段 `field` 在旧版本 `version` 里**还是不是同一个字段**。
///
/// 名字加类型文本**不足以**判定身份：一个名字可以被改名搬走之后又被新的
/// `Add` 重新占用。实证链条（`0.1 => 0.2 { n: i32 => m: i32 }` 后接
/// `0.2 => 0.3 { + n: i32 }`）：0.1 的 `n` 就是被改名成 `m` 的那个字段，
/// 而 0.3 的 `n` 是全新的——两者同名、同型，却毫无关系。把 0.3 的
/// `#[serde(default)]` 搬给 0.1 的 `n`，会让一个必需的旧字段变成可缺省，
/// 载荷里那个值被静默丢弃。所以身份必须按**来源**判定。
///
/// 做法：从当前版本沿链往回走到 `version`，在当前名字上逐个 hop 反推
/// （hop 的 `changes` 描述的是 `from` → `to`，`to` 一侧才是我们手上这一版）：
/// - `Add { value: (name, ..) }` 且 `name == 当前名`：这一版的字段是刚加出来的，
///   `from` 侧根本没有它 → **不是**同一个字段。
/// - `Rename`/`Copy` 且 `target == 当前名`：这一版的字段是由别的字段产出的
///   → 在 `from` 侧要跟着 `source` 走：单源继续用源名，多源则已无单一来源
///   → **不是**同一个字段。
/// - 其余情况：字段名在这一跳原样穿过，继续用同一个名字往更早走。
fn same_field_at(
    field: &Ident,
    version: &str,
    final_version: &str,
    hops: &[(String, Vec<MigrationField>, String)],
) -> bool {
    // 当前版本自己：就是同一个字段。
    let mut current = field.to_string();
    let mut at = final_version.to_owned();

    while at != version {
        // 找 `to == at` 的那一跳（链是位置性的，与 `lib.rs` 的回溯同款）。
        let Some((from, changes, _)) = hops.iter().find(|(_, _, to)| *to == at) else {
            // 走不到目标版本：链是断的，无法证明是同源，按「不是」处理
            // （宁可少搬属性——少搬只会让旧字段保持必需）。
            return false;
        };

        for change in changes {
            match change {
                MigrationField::Add { value, .. } => {
                    if value.0 == current {
                        // 这一版才加出来的字段，`from` 侧不存在。
                        return false;
                    }
                }
                MigrationField::Rename { source, target, .. }
                | MigrationField::Copy { source, target, .. } => {
                    if target.0 == current {
                        // 由源字段产出；只有单源才能继续追踪身份。
                        let [(source_ident, _)] = source.as_slice() else {
                            return false;
                        };
                        current = source_ident.to_string();
                    }
                }
                MigrationField::Remove { value, .. } => {
                    // `Remove` 在 `from` 侧**重新出现**该字段：从 `to` 看过去
                    // 它是被删掉的那个，不是穿过来的同一个字段。
                    if value.0 == current {
                        return false;
                    }
                }
            }
        }

        at = from.clone();
    }

    true
}

/// 某个旧版本结构体上该字段应当带哪些透传属性。
///
/// 两个条件**同时**成立才搬运：
/// 1. 按来源判定，它在那一版是同一个字段（见 [`same_field_at`]）；
/// 2. 它在那里的类型与今天的类型逐 token 相同。
///
/// 条件 2 单独不成立也不安全（同名不同型是另一个字段），单独成立也不安全
/// （同名同型但来源不同，见 [`same_field_at`] 的实证）；两者一起才足够。
fn attrs_for_older_field<'a>(
    attrs: &'a FieldPassthrough,
    current_types: &BTreeMap<Ident, Type>,
    final_version: &str,
    version: &str,
    hops: &[(String, Vec<MigrationField>, String)],
    ident: &Ident,
    older_ty: &Type,
) -> &'a [Attribute] {
    let Some(field_attrs) = attrs.get(ident) else {
        return &[];
    };

    if !same_field_at(ident, version, final_version, hops) {
        return &[];
    }

    // 同源之后再看类型：同名同型才是「同一个字段的同一份 schema」。
    let same_type = current_types.get(ident).is_some_and(|current_ty| {
        current_ty.to_token_stream().to_string() == older_ty.to_token_stream().to_string()
    });

    if same_type {
        field_attrs
    } else {
        &[]
    }
}

pub(crate) fn infer_older_version_struct(
    newer_struct_fields: BTreeMap<Ident, Type>,
    convert_rules: Vec<MigrationField>,
) -> Result<BTreeMap<Ident, Type>> {
    let mut struct_fields = newer_struct_fields.clone();

    for rule in convert_rules.iter() {
        match rule {
            MigrationField::Add { value, .. } => {
                let (key, _) = value;
                struct_fields.remove(key);
            }
            MigrationField::Copy { source, target, .. } => {
                let (target_ident, _) = target;
                struct_fields.remove(target_ident);

                for (ident, ty) in source.iter() {
                    struct_fields.insert(ident.clone(), ty.clone());
                }
            }
            MigrationField::Remove { value, .. } => {
                let (ident, ty) = value;
                struct_fields.insert(ident.clone(), ty.clone());
            }
            MigrationField::Rename { source, target, .. } => {
                let (target_ident, _) = target;
                struct_fields.remove(target_ident);

                for (ident, ty) in source.iter() {
                    struct_fields.insert(ident.clone(), ty.clone());
                }
            }
        }
    }

    Ok(struct_fields.into_iter().collect())
}

pub(crate) fn generate_old_versions(
    final_version: String,
    final_struct_fields: BTreeMap<Ident, Type>,
    versions: Vec<(String, Vec<MigrationField>, String)>,
) -> Result<Vec<(String, BTreeMap<Ident, Type>)>> {
    let mut temp_struct_fields = final_struct_fields.to_owned();
    let mut old_version_structs = vec![(final_version, temp_struct_fields.clone())];

    for (from_version, convert_rules, _to_version) in versions.iter() {
        temp_struct_fields =
            infer_older_version_struct(temp_struct_fields.clone(), convert_rules.to_owned())?;
        old_version_structs.push((from_version.clone(), temp_struct_fields.clone()));
    }

    Ok(old_version_structs)
}

pub(crate) fn generate_old_version_structs(
    ident: Ident,
    final_version: String,
    final_struct_fields: BTreeMap<Ident, Type>,
    extra_macros: Vec<TokenStream>,
    field_attrs: FieldPassthrough,
    versions: Vec<(String, Vec<MigrationField>, String)>,
) -> Result<TokenStream> {
    let old_version_structs = generate_old_versions(
        final_version.clone(),
        final_struct_fields.clone(),
        versions.clone(),
    )?;

    let old_version_structs = old_version_structs
        .iter()
        .map(|(version, fields)| {
            let struct_name = generate_ident(&ident, version)?;
            let fields = fields.iter().map(|(ident, ty)| {
                // 字段级透传属性只搬到「同源且同型」的字段上，判据见
                // [`attrs_for_older_field`]。当期版本的最终结构体落在
                // `final_version` 那一份（`fields` 与源结构体逐字一致），
                // 因此那里必然全量搬运。
                let attrs = attrs_for_older_field(
                    &field_attrs,
                    &final_struct_fields,
                    &final_version,
                    version,
                    &versions,
                    ident,
                    ty,
                );
                quote! {
                    #(#attrs)*
                    #ident: #ty,
                }
            });

            Ok(quote! {
                #[allow(non_camel_case_types, unused_variables, dead_code)]
                #(#extra_macros)*
                #[derive(::serde::Serialize, ::serde::Deserialize)]
                struct #struct_name {
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
