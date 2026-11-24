use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use syn::{
    bracketed, parenthesized,
    parse::{Parse, ParseStream},
    Attribute, Ident, ItemStruct, Token,
};

use super::MigrationComment;

#[derive(Clone)]
pub struct Migration {
    pub versions: Vec<MigrationComment>,
    pub extra_macros: Vec<(Ident, TokenStream)>,
    pub struct_data: ItemStruct,
}

/// 逐字段透传给**每一份**生成副本（隐藏的当期明文结构体 + 各隐藏旧版本
/// 结构体）的属性白名单。
///
/// 只放行 serde 语义属性：宏自行为每个版本生成了 `Serialize`/`Deserialize`
/// 实现（所以用户无法再 `#[derive(Serialize)]`，见 E0119），字段级 serde 属性
/// 因此必须由宏自己搬运——否则 `#[serde(default)]`、`#[serde(rename)]`、
/// `#[serde(flatten)]` 这类配置会被静默丢弃，或者只落在某一份副本上、
/// 与迁移链里解析旧数据的隐藏结构体不一致。`cfg`/`doc` 是纯元数据属性，
/// 不影响编解码，一并透传以免 cargo doc / cfg 行为飘移。
///
/// 刻意**不**透传 `specta`：specta 的字段属性只在类型本身 `#[derive(Type)]`
/// 时合法，透传到隐藏结构体（未派生 Type）会变成未知属性编译错误。
fn is_passthrough_attr(attr: &Attribute) -> bool {
    attr.path().is_ident("serde") || attr.path().is_ident("cfg") || attr.path().is_ident("doc")
}

impl Parse for Migration {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut versions = vec![];
        let mut extra_macros = vec![];

        while input.peek(Token![#]) {
            input.parse::<Token![#]>()?;

            let content;
            bracketed!(content in input);
            let key = content.parse::<Ident>()?;

            if key == "migration" {
                let inner_content;
                parenthesized!(inner_content in content);
                let item = inner_content.parse::<MigrationComment>()?;

                versions.push(item);
            } else {
                let tokens: TokenStream = content.parse()?;

                extra_macros.push((key, tokens));
            }
        }

        let struct_data: ItemStruct = input.parse()?;

        Ok(Self {
            versions,
            extra_macros,
            struct_data,
        })
    }
}

impl Migration {
    /// 收集字段级透传属性，键 = 字段名。
    ///
    /// 只收集白名单内的属性（见 [`is_passthrough_attr`]）；没有白名单属性的
    /// 字段不进表。字段顺序与 `struct_data` 一致，故同一张表可以按字段名
    /// 同时用于当前结构体与隐藏旧版本结构体。
    pub fn field_passthrough_attrs(&self) -> BTreeMap<Ident, Vec<Attribute>> {
        let mut map = BTreeMap::new();

        for field in self.struct_data.fields.iter() {
            let Some(ident) = field.ident.as_ref() else {
                continue;
            };
            let attrs: Vec<Attribute> = field
                .attrs
                .iter()
                .filter(|attr| is_passthrough_attr(attr))
                .cloned()
                .collect();

            if !attrs.is_empty() {
                map.insert(ident.clone(), attrs);
            }
        }

        map
    }
}
