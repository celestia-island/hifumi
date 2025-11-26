use syn::{
    parse::{Parse, ParseStream},
    Expr,
};

/// 版本属性的解析结果
///
/// 支持以下两种形式：
/// - `#[version("0.1")]` - 显式指定版本
/// - `#[version]` - 自动使用 `CARGO_PKG_VERSION` 环境变量
#[derive(Debug, Clone)]
pub struct DeriveVersion {
    /// 版本字符串，如果为 None 则使用 CARGO_PKG_VERSION
    pub version: Option<String>,
}

impl DeriveVersion {
    /// 获取版本字符串，如果未指定则使用 CARGO_PKG_VERSION
    pub fn get_version(&self) -> String {
        self.version.clone().unwrap_or_else(|| {
            std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string())
        })
    }
}

impl Parse for DeriveVersion {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // 如果输入为空，则使用 CARGO_PKG_VERSION
        if input.is_empty() {
            return Ok(Self { version: None });
        }

        Ok(Self {
            version: Some(match input.parse::<Expr>()? {
                Expr::Lit(lit) => match lit.lit {
                    syn::Lit::Str(s) => s.value(),
                    _ => return Err(syn::Error::new(input.span(), "Expected a string literal")),
                },
                _ => return Err(syn::Error::new(input.span(), "Expected a string literal")),
            }),
        })
    }
}
