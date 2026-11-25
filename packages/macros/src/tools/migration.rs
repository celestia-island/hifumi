use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use syn::{
    bracketed, parenthesized,
    parse::{Parse, ParseStream},
    Attribute, Ident, ItemStruct, Token,
};

use super::MigrationComment;
use crate::template::old_version_structs::FieldPassthrough;

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
/// [`crate::template::old_version_structs::attrs_for_field_at`]。
///
/// 刻意不放行的三类，处置**并不相同**——只有前两类会报错：
/// - `cfg`：**报错**。cfg 会按编译单元裁剪字段，宏无法在生成的历史结构体上
///   重现同一组 cfg 条件，照搬会造出「public 结构体被裁掉、隐藏结构体还在」
///   的字段集错位。
/// - `serde(flatten)`：**报错**。理由是 hifumi 自身的机制，不是 serde 的限制
///   ——flatten 捕获的是**动态**键集，而每一份按版本结构体都由静态字段表生成，
///   动态键无从穿过迁移链、也无从为旧版本重新写出。被测过的是：没有这条拒绝时
///   属性会被静默丢弃（不是编译失败），于是用户以为自己写了个生效的 flatten。
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

    // 走全仓唯一的 [`serde_items`] 解析器：只认**裸标记** `flatten`，故
    // `rename = "flatten"` 这类字符串不会误判；嵌套列表（`rename(serialize = "x")`）
    // 也照常穿到底，`flatten` 写在其后不会被漏掉。
    if mentions_flag(&serde_items(attr), "flatten") {
        return Some(
            "`#[serde(flatten)]` is not supported on a #[version] struct: hifumi builds \
             each per-version struct from a static field list, and flatten's dynamic key \
             set cannot be carried through the migration chain or re-emitted for an older \
             version (this is a hifumi restriction, not a serde one)"
                .to_string(),
        );
    }
    None
}

/// `#[serde(...)]` 里的一项。
///
/// 这是全仓**唯一**的 serde 属性解析器：flatten 拒绝与 `default = "path"`
/// 取值都走它，不再各自手搓一遍。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SerdeItem {
    /// 裸标记：`flatten`、`default`。
    Flag(String),
    /// `name = "literal"`，第二个字段是字符串**内容**：`rename = "n2"`、
    /// `default = "seven"`、`skip_serializing_if = "Option::is_none"`。
    ///
    /// 存 `String` 而不是 `syn::LitStr`：`LitStr` 的 `Debug`/`PartialEq` 要
    /// syn 的 `extra-traits` 特性，本 crate 没有开；内容对判定已足够。
    Named(String, String),
    /// `name(..)`：`rename(serialize = "x")`、`bound(deserialize = "..")`。
    Nested(String, Vec<SerdeItem>),
}

/// 把一条 `#[serde(...)]` 拆成 [`SerdeItem`] 列表。
///
/// **递归下降读 token 流**，不用 `syn::parse_nested_meta`：后者的回调必须
/// 自己把每个条目的 token 消费干净，遇到 `rename(serialize = "x")` 这类嵌套
/// 列表时 `meta.value()` 会失败、遍历就此中断，其后的条目便永远看不到——
/// `#[serde(rename = "n2", default = "seven")]` 取不到 `default`、
/// `rename(serialize = "x"), flatten` 会漏掉 flatten，都是这个形态。手写
/// 下降对嵌套与条目顺序都不敏感。
///
/// 非 `serde` 属性、以及 `#[serde]`（无括号）返回空表。
pub(crate) fn serde_items(attr: &Attribute) -> Vec<SerdeItem> {
    if !attr.path().is_ident("serde") {
        return Vec::new();
    }
    let syn::Meta::List(list) = &attr.meta else {
        return Vec::new();
    };
    read_items(list.tokens.clone().into_iter().peekable())
}

type SerdeStream = std::iter::Peekable<proc_macro2::token_stream::IntoIter>;

/// 读一项：`ident`、`ident = "lit"` 或 `ident ( .. )`。
fn parse_serde_item(stream: &mut SerdeStream) -> Option<SerdeItem> {
    // 只在确实拿到 `Ident` 时才前进；遇到逗号之类的分隔符**不动指针**，
    // 交给调用方跳过。曾经在这里无条件 `next()`，于是分离符被吞掉后
    // 调用方又多跳一个 token，紧随其后的条目就整条丢了。
    match stream.peek() {
        Some(proc_macro2::TokenTree::Ident(_)) => {}
        _ => return None,
    }
    let Some(proc_macro2::TokenTree::Ident(ident)) = stream.next() else {
        return None;
    };
    let name = ident.to_string();

    // `ident ( .. )`：嵌套列表，递归读进一个 `Vec`。
    if let Some(proc_macro2::TokenTree::Group(group)) = stream.peek() {
        if group.delimiter() == proc_macro2::Delimiter::Parenthesis {
            let group = match stream.next() {
                Some(proc_macro2::TokenTree::Group(g)) => g,
                _ => return Some(SerdeItem::Flag(name)),
            };
            let inner = read_items(group.stream().into_iter().peekable());
            return Some(SerdeItem::Nested(name, inner));
        }
    }

    // `ident = "lit"`：只认等号后紧跟字符串字面量；否则视为裸标记。
    if let Some(proc_macro2::TokenTree::Punct(punct)) = stream.peek() {
        if punct.as_char() == '=' {
            stream.next();
            if let Some(proc_macro2::TokenTree::Literal(literal)) = stream.peek() {
                if let Ok(lit) = syn::parse_str::<syn::LitStr>(&literal.to_string()) {
                    stream.next();
                    return Some(SerdeItem::Named(name, lit.value()));
                }
            }
            return Some(SerdeItem::Flag(name));
        }
    }

    Some(SerdeItem::Flag(name))
}

/// 唯一的读循环：读得动就收下，读不动就**跳过这个 token 接着读**。
///
/// 「跳过而不是停下」是这里的全部要点——一旦提前 `break`，后面的条目就
/// 永远看不见（这正是 `parse_nested_meta` 行为的形态）。
fn read_items(mut stream: SerdeStream) -> Vec<SerdeItem> {
    let mut items = Vec::new();
    while stream.peek().is_some() {
        if let Some(item) = parse_serde_item(&mut stream) {
            items.push(item);
        } else {
            stream.next();
        }
    }
    items
}

/// 递归查找某个名字的**裸标记**（任意层嵌套里）。
fn mentions_flag(items: &[SerdeItem], needle: &str) -> bool {
    items.iter().any(|item| match item {
        SerdeItem::Flag(name) => name == needle,
        SerdeItem::Named(..) => false,
        SerdeItem::Nested(_, inner) => mentions_flag(inner, needle),
    })
}

/// 递归查找某个名字的 `= "literal"` 取值（任意层嵌套里，先到先得）。
fn find_named<'a>(items: &'a [SerdeItem], needle: &str) -> Option<&'a str> {
    for item in items {
        match item {
            SerdeItem::Named(name, value) if name == needle => return Some(value.as_str()),
            SerdeItem::Nested(_, inner) => {
                if let Some(value) = find_named(inner, needle) {
                    return Some(value);
                }
            }
            _ => {}
        }
    }
    None
}

/// 取字段上的 `#[serde(default = "path")]` 的函数路径。
///
/// 裸 `#[serde(default)]` 返回 `None`——那是「取 `Default::default()`」的意思。
/// 每个 `#[serde(...)]` 条目都经 [`serde_items`] 完整解析，故 `default` 写在
/// `rename` 前面还是后面都一样。
pub(crate) fn serde_default_path(attrs: &[Attribute]) -> Option<String> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("serde"))
        .find_map(|attr| find_named(&serde_items(attr), "default").map(str::to_owned))
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
    pub fn field_passthrough_attrs(&self) -> syn::Result<FieldPassthrough> {
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

        Ok(FieldPassthrough::new(map))
    }
}

#[cfg(test)]
mod serde_walker_tests {
    use super::*;

    fn items(src: &str) -> Vec<SerdeItem> {
        serde_items(&attr(src))
    }

    /// `syn::Attribute` 本身不实现 `Parse`，借一个带该属性的 item 把它取出来。
    fn attr(src: &str) -> Attribute {
        let mut source = String::from(src);
        source.push_str("\nfn _holder() {}");
        let item: syn::ItemFn =
            syn::parse_str(&source).unwrap_or_else(|e| panic!("{source:?} must parse: {e}"));
        item.attrs
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("{src} must carry one attribute"))
    }

    fn flag(name: &str) -> SerdeItem {
        SerdeItem::Flag(name.to_string())
    }

    fn named(name: &str, value: &str) -> SerdeItem {
        SerdeItem::Named(name.to_string(), value.to_string())
    }

    /// 裸标记。
    #[test]
    fn bare_flag() {
        assert_eq!(items(r#"#[serde(flatten)]"#), vec![flag("flatten")]);
        assert_eq!(items(r#"#[serde(default)]"#), vec![flag("default")]);
    }

    /// 嵌套列表之后**还有**条目：这正是 `parse_nested_meta` 会中断、手搓
    /// 扫描能穿过去的那种形态。
    #[test]
    fn nested_list_does_not_truncate_the_walk() {
        assert_eq!(
            items(r#"#[serde(rename(serialize = "x"), flatten)]"#),
            vec![
                SerdeItem::Nested("rename".to_string(), vec![named("serialize", "x")],),
                flag("flatten"),
            ]
        );
    }

    /// 字符串里的 `flatten` 不是标记——`rename = "flatten"` 与 `alias = "flatten"`
    /// 都不得被判定成 flatten。
    #[test]
    fn strings_are_not_flags() {
        for src in [
            r#"#[serde(rename = "flatten")]"#,
            r#"#[serde(alias = "flatten")]"#,
        ] {
            let parsed = items(src);
            assert_eq!(parsed.len(), 1, "{src} -> {parsed:?}");
            assert!(
                !mentions_flag(&parsed, "flatten"),
                "{src} must not count as flatten: {parsed:?}"
            );
        }
    }

    /// `default = "path"` 取得到；裸 `default` 取不到（= 用 `Default::default()`）。
    #[test]
    fn default_path_forms() {
        assert_eq!(
            items(r#"#[serde(default = "one")]"#),
            vec![named("default", "one")]
        );
        assert_eq!(items(r#"#[serde(default)]"#), vec![flag("default")]);
        assert_eq!(find_named(&items(r#"#[serde(default)]"#), "default"), None);
        assert_eq!(
            find_named(&items(r#"#[serde(default = "one")]"#), "default"),
            Some("one")
        );
    }

    /// **顺序无关**：`default` 写在 `rename` 前还是后都必须看得到。
    ///
    /// 这是 R16 的原始缺陷——`#[serde(rename = "n2", default = "seven")]`
    /// 在旧实现下取不到 `default`（遍历在 `rename` 处中断）。
    #[test]
    fn order_does_not_matter() {
        for src in [
            r#"#[serde(rename = "n2", default = "seven")]"#,
            r#"#[serde(default = "seven", rename = "n2")]"#,
        ] {
            let parsed = items(src);
            assert_eq!(
                find_named(&parsed, "default"),
                Some("seven"),
                "{src} -> {parsed:?}"
            );
            assert_eq!(find_named(&parsed, "rename"), Some("n2"), "{src}");
        }
    }

    /// 其它常见取值形态照常读出，且不被误当成标记。
    #[test]
    fn other_value_forms() {
        let skip = items(r#"#[serde(skip_serializing_if = "Option::is_none")]"#);
        assert_eq!(
            find_named(&skip, "skip_serializing_if"),
            Some("Option::is_none")
        );
        assert!(!mentions_flag(&skip, "flatten"));

        let with = items(r#"#[serde(with = "m")]"#);
        assert_eq!(find_named(&with, "with"), Some("m"));
        assert!(!mentions_flag(&with, "flatten"));
    }

    /// 非 serde 属性、以及无括号的 `#[serde]` 都返回空表，不 panic。
    #[test]
    fn non_serde_attributes_yield_nothing() {
        assert!(serde_items(&attr("/// hi")).is_empty());
        assert!(serde_items(&attr("#[serde]")).is_empty());
    }

    /// flatten 检测走嵌套层：`rename(serialize = "x"), flatten` 必须命中。
    #[test]
    fn flatten_is_found_through_nesting() {
        assert!(mentions_flag(
            &items(r#"#[serde(rename(serialize = "x"), flatten)]"#),
            "flatten"
        ));
        assert!(mentions_flag(&items(r#"#[serde(flatten)]"#), "flatten"));
        assert!(!mentions_flag(
            &items(r#"#[serde(rename = "x")]"#),
            "flatten"
        ));
    }
}
