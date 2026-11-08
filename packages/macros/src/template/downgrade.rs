use anyhow::Result;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::collections::BTreeMap;
use syn::{Ident, LitStr, Type};

use crate::{tools::MigrationComment, utils::generate_ident};

use super::impl_migration::step_missing_back;
use super::old_version_structs::infer_older_version_struct;

/// 生成 `#ident::to_version_value(&self, target) -> Result<Value, DowngradeError>`。
///
/// 每个历史版本一个 match 臂：链上自 final 起全部步骤可逆 → 逐级嵌套反向
/// `From` 求出目标版本表示并交给运行时 `__downgrade_json` 注入 `$version`；
/// 中途碰到不可逆步 → 该臂与其后所有臂返回 `DowngradeError::Irreversible`
/// （带第一个缺 `back` 的步与字段）。未知版本 → `UnknownTarget`。
pub(crate) fn generate_downgrade_impl(
    ident: &Ident,
    final_version: String,
    final_struct_fields: &BTreeMap<Ident, Type>,
    versions: Vec<MigrationComment>,
) -> Result<TokenStream> {
    let final_repr = generate_ident(ident, &final_version)?;
    let final_lit = LitStr::new(&final_version, Span::call_site());

    // 当前版本：恒可达，等价于普通序列化
    let mut arms = vec![quote! {
        #final_lit => {
            ::hifumi::__downgrade_json(#final_repr::from(self.clone()), #final_lit)
        }
    }];

    // 与 impl_migration 相同的回溯与环防护；同步维护「新」侧字段表，
    // 供 step_missing_back 判断纯改名的目标是否存活到新结构体
    let mut temp_version = final_version.clone();
    let mut temp_struct_fields = final_struct_fields.to_owned();
    let mut visited = std::collections::HashSet::new();
    // 从 `self.clone()`（#ident）出发的反向链表达式；每过一个可逆步包一层 From
    let mut chain: TokenStream = quote! { self.clone() };
    let mut blocked: Option<(String, String, String)> = None;

    while let Some(item) = versions.iter().find(|item| item.to.value() == temp_version) {
        let to = item.to.value();
        if !visited.insert(to.clone()) {
            return Err(anyhow::anyhow!(
                "cycle in migration chain: version {to:?} is revisited; \
                 each migration must move strictly backwards"
            ));
        }

        let from_lit = LitStr::new(&item.from.value(), Span::call_site());
        let newer_struct_fields = temp_struct_fields.clone();
        temp_struct_fields =
            infer_older_version_struct(temp_struct_fields.clone(), item.changes.clone())?;
        if blocked.is_none() {
            if let Some(field) = step_missing_back(&item.changes, &newer_struct_fields) {
                blocked = Some((item.from.value(), item.to.value(), field));
            } else {
                let from_ident = generate_ident(ident, item.from.value())?;
                chain = quote! { #from_ident::from(#chain) };
            }
        }

        let arm = match &blocked {
            None => quote! {
                #from_lit => { ::hifumi::__downgrade_json(#chain, #from_lit) }
            },
            Some((from, to, field)) => quote! {
                #from_lit => {
                    Err(::hifumi::DowngradeError::Irreversible {
                        from: #from.to_owned(),
                        to: #to.to_owned(),
                        field: #field.to_owned(),
                    })
                }
            },
        };
        arms.push(arm);

        temp_version = item.from.value();
    }

    Ok(quote! {
        #[automatically_derived]
        impl #ident {
            /// 将当前值降级为 `target` 版本的带 `$version` 标签 JSON 值。
            ///
            /// 仅当自当前版本到 `target` 的每一步迁移都免费可逆（加字段 /
            /// 复制 / 纯改名）或声明了 `back` 表达式时成功；否则返回
            /// [`hifumi::DowngradeError::Irreversible`]，指明第一个缺
            /// 反向转换的步与字段。
            pub fn to_version_value(
                &self,
                target: &str,
            ) -> ::core::result::Result<::hifumi::serde_json::Value, ::hifumi::DowngradeError> {
                match target {
                    #(#arms)*
                    _ => Err(::hifumi::DowngradeError::UnknownTarget {
                        type_name: ::core::stringify!(#ident).to_owned(),
                        target: target.to_owned(),
                    }),
                }
            }
        }
    })
}
