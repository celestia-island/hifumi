use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::ToTokens;
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

/// 逐字段透传给生成副本（隐藏的当期明文结构体 + 各隐藏旧版本结构体）的
/// 属性白名单。
///
/// 宏自行为每个版本生成了 `Serialize`/`Deserialize` 实现（所以用户无法再
/// `#[derive(Serialize)]`，见 E0119），字段级 serde 属性因此必须由宏自己搬运
/// ——否则 `#[serde(default)]`、`#[serde(rename)]` 这类配置会被静默丢弃，
/// 或者只落在某一份副本上、与迁移链里解析旧数据的隐藏结构体不一致。
///
/// 表以字段名为键，因此**不是**「按名字无条件搬运」：搬运到旧版本结构体时
/// 还要同名同型才成立，判据在
/// [`crate::template::old_version_structs::attrs_for_older_field`]。
///
/// 刻意不放行的三类，处置**并不相同**——只有前两类会报错：
/// - `cfg`：**报错**。cfg 会按编译单元裁剪字段，宏无法在生成的历史结构体上
///   重现同一组 cfg 条件，照搬会造出「public 结构体被裁掉、隐藏结构体还在」
///   的字段集错位。
/// - `serde(flatten)`：**报错**。内部标签枚举（`$version`）的载荷不允许
///   flatten，在这里放行等于让用户写一个必然编译失败的属性。
/// - `specta(...)`：**静默丢弃**（不进白名单，也不报错）。字段级 specta 属性
///   只在类型本身 `#[derive(Type)]` 时合法，透传到未派生 Type 的隐藏结构体
///   会变成未知属性编译错误；但宏也无法断定用户是不是想表达别的意思，故只
///   是不搬运。`doc` 是纯注释，照常透传。
fn field_attr_rejection(attr: &Attribute) -> Option<String> {
    if attr.path().is_ident("cfg") {
        return Some(
            "`#[cfg]` on a field of a #[version] struct is not supported: the macro \
             cannot reproduce the same cfg set on every generated per-version struct"
                .to_string(),
        );
    }

    if !attr.path().is_ident("serde") {
        return None;
    }

    // Token 扫描而不是 `parse_nested_meta`：后者的回调必须自己消费掉每个条目的
    // token，遇到 `rename(serialize = "x")` 这类嵌套列表时 `meta.value()` 失败、
    // 遍历就此中断，其后的 `flatten` 便逃过拒绝（实测可复现）。扫描 `Ident` 不会
    // 误伤字符串里的同名文本——`"flatten"` 是 `Literal` 而不是 `Ident`。
    if token_stream_mentions_ident(attr.meta.to_token_stream(), "flatten") {
        return Some(
            "`#[serde(flatten)]` is not supported on a #[version] struct: the payload \
             of the `$version` internally tagged enum cannot be flattened"
                .to_string(),
        );
    }
    None
}

/// 递归判断一个 token 流里是否出现给定的 `Ident`（`Group` 内部也算）。
///
/// 只认 `Ident`：字符串字面量（如 `rename = "flatten"`）是 `Literal`，不会命中。
fn token_stream_mentions_ident(stream: proc_macro2::TokenStream, needle: &str) -> bool {
    stream.into_iter().any(|tree| match tree {
        proc_macro2::TokenTree::Ident(ident) => ident == needle,
        proc_macro2::TokenTree::Group(group) => token_stream_mentions_ident(group.stream(), needle),
        _ => false,
    })
}

/// 白名单：**只有** serde 与 doc 会被搬运。
///
/// 它只决定「搬什么」；「拒什么」由 [`field_attr_rejection`] 决定，两者不是
/// 补集关系——`cfg` 在白名单外且会报错，`specta(...)` 在白名单外且**静默丢弃**。
fn is_passthrough_attr(attr: &Attribute) -> bool {
    attr.path().is_ident("serde") || attr.path().is_ident("doc")
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
    ///
    /// 明确不支持的字段属性在**这里**报错并指到字段自己的 span：静默丢弃
    /// 会让用户以为 `#[serde(flatten)]` 生效了，而它其实什么也没做。
    pub fn field_passthrough_attrs(&self) -> syn::Result<BTreeMap<Ident, Vec<Attribute>>> {
        let mut map = BTreeMap::new();

        for field in self.struct_data.fields.iter() {
            let Some(ident) = field.ident.as_ref() else {
                continue;
            };
            for attr in field.attrs.iter() {
                if let Some(reason) = field_attr_rejection(attr) {
                    return Err(syn::Error::new_spanned(attr, reason));
                }
            }
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

        Ok(map)
    }
}
