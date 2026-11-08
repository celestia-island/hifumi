use syn::{
    parse::{Parse, ParseStream},
    Ident, Token,
};

/// 版本属性的解析结果
///
/// 支持以下形式：
/// - `#[version("0.1")]` - 显式指定版本
/// - `#[version]` - 自动使用 `CARGO_PKG_VERSION` 环境变量
/// - `#[version("0.1", forward)]` / `#[version(forward)]` - 追加前向兼容
///   旗标：未知（更新）版本的数据按宽容读者语义解析（见 `hifumi::Forward`）
#[derive(Clone)]
pub struct DeriveVersion {
    /// 版本字符串，如果为 None 则使用 CARGO_PKG_VERSION
    pub version: Option<String>,
    /// 前向兼容旗标；开启后生成的 `Deserialize` 对未知 `$version`
    /// 走宽容回退而不是直接报错
    pub forward: bool,
}

impl DeriveVersion {
    /// 获取版本字符串，如果未指定则使用 CARGO_PKG_VERSION
    ///
    /// Audit hifumi-R1-3: env 缺失时显式报错而非静默钉 0.0.0——静默
    /// 回退会让数据以错误版本号落盘，事后修正版本号即 unknown variant。
    pub fn get_version(&self) -> anyhow::Result<String> {
        if let Some(v) = &self.version {
            return Ok(v.clone());
        }
        std::env::var("CARGO_PKG_VERSION").map_err(|e| {
            anyhow::anyhow!(
                "CARGO_PKG_VERSION is not set and no explicit version was given to \
                 #[version]: the struct would be silently pinned to 0.0.0"
            )
            .context(e)
        })
    }
}

impl Parse for DeriveVersion {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut version = None;
        let mut forward = false;

        while !input.is_empty() {
            // 版本字面量永远以字符串字面量开头；裸标识符只可能是具名旗标
            if input.peek(syn::LitStr) {
                if version.is_some() {
                    return Err(syn::Error::new(
                        input.span(),
                        "duplicate version literal in #[version(...)]",
                    ));
                }
                version = Some(input.parse::<syn::LitStr>()?.value());
            } else {
                let ident: Ident = input.parse()?;
                if ident != "forward" {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!(
                            "unknown flag `{ident}`; expected `forward` or a version string \
                             literal"
                        ),
                    ));
                }
                if forward {
                    return Err(syn::Error::new(
                        ident.span(),
                        "duplicate `forward` flag in #[version(...)]",
                    ));
                }
                forward = true;
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self { version, forward })
    }
}
