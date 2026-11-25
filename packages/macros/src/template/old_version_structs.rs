use anyhow::Result;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use std::collections::BTreeMap;
use syn::{Attribute, Ident, Type};

use crate::{tools::MigrationField, utils::generate_ident};

/// 字段名 → 逐字段透传属性（见 [`crate::tools::Migration::field_passthrough_attrs`]）。
/// 单字段没有白名单属性时，`get` 落空即不生成任何 attribute。
///
/// # 消费方清单（加新消费方前必读）
///
/// 这张表按**当前字段名**为键，而「这个名字在别的版本里是不是同一个字段」是
/// 另一个问题——直接 `attrs.get(..)` 就是在假设答案是「是」。因此所有消费方
/// 都必须经 [`attrs_for_field_at`] 取属性，它同时把关来源与类型：
///
/// 1. `generate_old_version_structs`：给每份隐藏旧版本结构体生成字段时。
/// 2. `impl_migration::generate_older_version_impl`：`Add` 无转换器时决定补什么
///    缺省值（`serde(default = "..")` 还是 `Default::default()`）。
///
/// 两次事故都出自「漏掉这道关」：R16 是来源判定只比名字，R19 是第 2 个消费方
/// 按裸名字取值。**再有第三个消费方，也必须走 [`attrs_for_field_at`]。**
pub(crate) type FieldPassthrough = BTreeMap<Ident, Vec<Attribute>>;

/// 当前版本的字段 `field` 一路回溯到版本 `version` 时，**在那个版本里叫什么**。
///
/// 返回 `Some(name)` 表示「同一个字段在那个版本里叫 `name`」；`None` 表示
/// 它在那版本里根本不存在。**只回答「是不是同一个字段」是不够的**：字段可能
/// 在中间某跳被改名，于是在旧版本里的名字与今天不同（甚至与今天另一个字段
/// 重名）。调用方必须拿这个**终点名字**去比对，而不是拿当前名字。
///
/// 名字加类型文本都不足以判定身份：一个名字可以被改名搬走之后又被新的 `Add`
/// 重新占用。实证链条（`0.1 => 0.2 { n: i32 => m: i32 }` 后接
/// `0.2 => 0.3 { + n: i32 }`）：0.1 的 `n` 就是被改名成 `m` 的那个字段，
/// 而 0.3 的 `n` 是全新的——两者同名、同型，却毫无关系。更隐蔽的一种
/// （R16）：`0.2 => 0.3 { w: i32 => x: i32 }` 配 `0.1 => 0.2 { x: i32 => y: i32 }`，
/// 0.3 的 `x` 在线系上是 0.1 的 `w`，而 0.1 的 `x` 其实是 0.3 的 `y`——
/// 若只带回一个 bool，属性就会落到 0.1 那个**同名的另一个字段**上。
///
/// 做法：从当前版本沿链往回走到 `version`，在当前名字上逐个 hop 反推
/// （hop 的 `changes` 描述的是 `from` → `to`，`to` 一侧才是我们手上这一版）：
/// - `Add { value: (name, ..) }` 且 `name == 当前名`：这一版的字段是刚加出来的，
///   `from` 侧根本没有它 → `None`。
/// - `Rename`/`Copy` 且 `target == 当前名`：这一版的字段由别的字段产出 →
///   跟着**单源**继续往早走；多源则已无单一来源 → `None`。
/// - `Remove { value: (name, ..) }` 且 `name == 当前名`：`Remove` 让该字段在
///   `from` 侧重新出现，它是被删掉的那个，不是穿过来的同一个 → `None`。
/// - 其余情况：字段名在这一跳原样穿过。
///
/// 残留边界：判定只沿**单条名字线**走，一拍里同时改两个字段的写法（把一个
/// 名字腾给另一个字段、再从第三个字段改名接过来）无法表达成一条 `Rename`，
/// 因而不可能被这条规则误判——需要那种语义时用户只能写成两拍，而两拍里
/// 每一拍都仍然是单源改名。
fn field_name_at(
    field: &Ident,
    version: &str,
    final_version: &str,
    hops: &[(String, Vec<MigrationField>, String)],
) -> Option<String> {
    // 当前版本自己：就是它自己的名字。
    let mut current = field.to_string();
    let mut at = final_version.to_owned();

    while at != version {
        // 找 `to == at` 的那一跳（链是位置性的，与 `lib.rs` 的回溯同款）。
        let Some((from, changes, _)) = hops.iter().find(|(_, _, to)| *to == at) else {
            // 走不到目标版本：链是断的，无法证明同源 → 当作不存在。
            return None;
        };

        for change in changes {
            match change {
                MigrationField::Add { value, .. } => {
                    if value.0 == current {
                        return None;
                    }
                }
                MigrationField::Rename { source, target, .. }
                | MigrationField::Copy { source, target, .. } => {
                    if target.0 == current {
                        let [(source_ident, _)] = source.as_slice() else {
                            return None;
                        };
                        current = source_ident.to_string();
                    }
                }
                MigrationField::Remove { value, .. } => {
                    if value.0 == current {
                        return None;
                    }
                }
            }
        }

        at = from.clone();
    }

    Some(current)
}

/// 字段 `ident` 在版本 `version` 上应当带哪些透传属性——**唯一**的判定入口。
///
/// 全仓只有这一处决定「这个字段配不配这些属性」。任何新的消费方都必须走
/// 这里，不要再直接 `attrs.get(..)`：R19 就是因为另一个消费方（`Add` 无转换器
/// 时取 `serde(default = "..")`）绕过了判定、按裸名字取值，让一个已经改名为
/// `k` 的字段偷走了当前字段的 `seven`。
///
/// 判据两步，缺一不可：
/// 1. **来源**：沿迁移链回溯到 `version`，字段还在，且终点名仍是 `ident`
///    （[`field_name_at`]）。终点名不同说明那一版叫这个名字的是**另一个**
///    字段，哪怕同名同型。
/// 2. **类型**：它在那里的类型与今天的类型逐 token 相同。同源之后类型仍可能
///    在某一版被改写（`n: i32 => n: String` 保名换型），那时 schema 已不同。
///
/// 这一步偏向「不给」，但「不给」的后果分两个方向，见 `generate_old_version_structs`
/// 里的落点注释。
pub(crate) fn attrs_for_field_at<'a>(
    attrs: &'a FieldPassthrough,
    current_types: &BTreeMap<Ident, Type>,
    final_version: &str,
    version: &str,
    hops: &[(String, Vec<MigrationField>, String)],
    ident: &Ident,
    ty_there: &Type,
) -> &'a [Attribute] {
    let Some(field_attrs) = attrs.get(ident) else {
        return &[];
    };

    // 必须拿**终点名字**比对：同源但改名过的字段，在那一版里叫别的名字。
    let Some(name_there) = field_name_at(ident, version, final_version, hops) else {
        return &[];
    };
    if name_there.as_str() != ident.to_string().as_str() {
        return &[];
    }

    let same_type = current_types.get(ident).is_some_and(|current_ty| {
        current_ty.to_token_stream().to_string() == ty_there.to_token_stream().to_string()
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
                // 字段级透传属性只搬到「同源、同名且同型」的字段上，判据见
                // [`attrs_for_field_at`]。当期版本的最终结构体落在
                // `final_version` 那一份（`fields` 与源结构体逐字一致），
                // 因此那里必然全量搬运。
                //
                // 不搬的后果**分两个方向**，不能一律说成「更安全」：
                // - `default`（裸的或带函数）：不搬 = 旧字段保持必需。损坏的旧
                //   载荷会报错而不是被补上默认值——这是**静默腐坏变响亮失败**。
                // - `rename`/`alias`/`skip_serializing_if`/`with` 等改写线格式的
                //   属性：不搬 = 旧版本结构体用**另一套** wire schema。改名键的
                //   旧载荷会解析失败（响亮），而生成本该被跳过的键也会出现
                //   （旧结构体的输出多字段）——这是**旧 schema 被改写**，不是
                //   「更严格」，应当当作破坏性变更对待。
                let attrs = attrs_for_field_at(
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
