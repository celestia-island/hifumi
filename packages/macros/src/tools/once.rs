use syn::{
    parse::{Parse, ParseStream},
    Ident, LitStr, Token,
};

/// 单次动作属性的解析结果
///
/// 支持以下形式：
/// - `#[once("0.5.2")]` - 升级跨入 0.5.2 时执行一次，来源版本不限
/// - `#[once("0.4.0" => "0.5.2")]` - 仅当从 0.4.0 及以后升级跨入 0.5.2 时执行
/// - 以上均可附加 `, delegate` 标记：动作不在本地执行，由外部执行者
///   （例如桌面应用的 WebView 前端）完成后回报记账，函数体应留空
#[derive(Clone)]
pub struct OnceAttr {
    /// 来源版本下限（`"from" => "since"` 的 `from`）。
    pub from: Option<LitStr>,
    /// 引入该动作的版本。
    pub since: LitStr,
    pub delegate: bool,
}

impl Parse for OnceAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // "0.5.2" / "0.4.0" => "0.5.2"（可选 `, delegate`）

        let first = input.parse::<LitStr>()?;
        let (from, since) = if input.peek(Token![=>]) {
            input.parse::<Token![=>]>()?;
            (Some(first), input.parse::<LitStr>()?)
        } else {
            (None, first)
        };

        let mut delegate = false;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let flag = input.parse::<Ident>()?;
            if flag != "delegate" {
                return Err(syn::Error::new(flag.span(), "Expected `delegate`"));
            }
            delegate = true;
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        if !input.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "Unexpected token in #[once(...)]",
            ));
        }

        Ok(Self {
            from,
            since,
            delegate,
        })
    }
}
