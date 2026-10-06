use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{ItemMod, LitStr};

use crate::tools::{DeriveVersion, OnceAttr};

/// 生成 `#[app_migrations]` 模块内的动作注册表：遍历模块里的函数项，
/// 剥离并解析每个 `#[once(...)]` 属性（其余属性与未标注的函数原样
/// 保留），追加一个隐藏的 `registry()` 构造函数。返回 `syn::Result`，
/// 让语法错误携带原始 span 编译报错，而不是 panic。
pub(crate) fn generate_app_migrations_module(
    attr: DeriveVersion,
    mut module: ItemMod,
) -> syn::Result<TokenStream> {
    let version = attr.get_version();

    let Some((brace, mut items)) = module.content.take() else {
        return Err(syn::Error::new(
            module.ident.span(),
            "Expected a module body after #[app_migrations]",
        ));
    };

    let mut entries = vec![];
    for item in items.iter_mut() {
        let syn::Item::Fn(func) = item else {
            continue;
        };

        let mut once = None;
        let mut kept_attrs = vec![];
        for attribute in func.attrs.drain(..) {
            if attribute.path().is_ident("once") {
                if once.is_some() {
                    return Err(syn::Error::new(
                        func.sig.ident.span(),
                        "Duplicate #[once] attribute on one function",
                    ));
                }
                once = Some(attribute.parse_args::<OnceAttr>()?);
            } else {
                kept_attrs.push(attribute);
            }
        }
        if let Some(attr) = &once {
            // 委托动作的函数体不参与生成代码，替它挂上 allow 避免
            // dead_code 告警。
            if attr.delegate {
                kept_attrs.push(syn::parse_quote! { #[allow(dead_code)] });
            }
        }
        func.attrs = kept_attrs;

        if let Some(attr) = once {
            entries.push((func.sig.ident.clone(), attr));
        }
    }

    let entry_tokens = entries
        .iter()
        .map(|(ident, attr)| {
            let id = LitStr::new(&ident.to_string(), Span::call_site());
            let since = &attr.since;
            let from = match &attr.from {
                Some(from) => quote! { ::core::option::Option::Some(#from) },
                None => quote! { ::core::option::Option::None },
            };
            // 类型标注让签名不符的函数在注册处得到清晰的编译错误，
            // 而不是隐晦的 fn 指针协变失败。
            let action = if attr.delegate {
                quote! { ::hifumi::app::AppMigrationAction::Delegated }
            } else {
                quote! {
                    ::hifumi::app::AppMigrationAction::Immediate({
                        let action: ::hifumi::app::AppActionFn = #ident;
                        action
                    })
                }
            };
            quote! {
                ::hifumi::app::AppMigrationEntry {
                    id: #id,
                    since: #since,
                    from: #from,
                    action: #action,
                }
            }
        })
        .collect::<Vec<_>>();

    let version_lit = LitStr::new(&version, Span::call_site());
    items.push(syn::parse_quote! {
        /// 由 `#[app_migrations]` 生成的动作注册表
        #[doc(hidden)]
        pub fn registry() -> ::hifumi::app::AppMigrationSet {
            ::hifumi::app::AppMigrationSet {
                current_version: #version_lit,
                entries: ::std::vec![#(#entry_tokens),*],
            }
        }
    });
    module.content = Some((brace, items));

    Ok(quote! { #module })
}
