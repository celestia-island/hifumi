//! 降级（当前版本 → 历史版本）的运行时支撑。
//!
//! `#[version]` 宏为每个版本化类型生成 `to_version_value(&self, target)`；
//! 本模块提供它的错误类型与 JSON 组装助手。降级一步的规则：
//! - `Add` / `Copy`：反向免费（丢弃新增字段，按设计有损）；
//! - 纯改名（单源、无变换、同类型）：反向免费；
//! - `Remove` / 带变换的 `Rename`：必须在迁移 DSL 里声明 `back` 表达式，
//!   否则该步（及其下方的所有更旧版本）不可逆。

use std::fmt;

/// `to_version_value` 的错误类型。
#[derive(Debug)]
#[non_exhaustive]
pub enum DowngradeError {
    /// 目标版本不在该类型的迁移链上。
    UnknownTarget {
        /// 版本化类型的名字（宏展开处生成）。
        type_name: String,
        /// 请求的目标版本串。
        target: String,
    },
    /// 链上存在缺 `back` 表达式的步骤，目标版本不可达。
    /// `from`/`to` 是该步迁移的 DSL 方向（旧 => 新），`field` 是第一个
    /// 缺反向转换的字段。
    Irreversible {
        /// 步骤的旧版本。
        from: String,
        /// 步骤的新版本。
        to: String,
        /// 第一个缺 `back` 的字段名（多源 rename 为逗号分隔列表）。
        field: String,
    },
    /// 序列化历史表示失败（字段类型无法转 JSON）。
    Serialize(serde_json::Error),
}

impl From<serde_json::Error> for DowngradeError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialize(error)
    }
}

impl fmt::Display for DowngradeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTarget { type_name, target } => write!(
                f,
                "version {target:?} is not on the migration chain of {type_name}"
            ),
            Self::Irreversible { from, to, field } => write!(
                f,
                "step {from:?} => {to:?} has no backward converter for field(s) {field:?}; \
                 declare `{{ back: <expr> }}` to make it reversible"
            ),
            Self::Serialize(error) => write!(f, "failed to serialize the historical view: {error}"),
        }
    }
}

impl std::error::Error for DowngradeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Serialize(error) => Some(error),
            _ => None,
        }
    }
}

/// 由宏生成的 `to_version_value` 调用的组装助手（doc(hidden)，非公共 API）：
/// 把目标版本的表示序列化成 JSON，并注入 `$version` 标签，得到与正向
/// 反序列化完全同构的载荷。
#[doc(hidden)]
pub fn __downgrade_json<T: serde::Serialize>(
    repr: T,
    version: &str,
) -> Result<serde_json::Value, DowngradeError> {
    let mut value = serde_json::to_value(&repr)?;
    match value {
        serde_json::Value::Object(ref mut map) => {
            map.insert(
                "$version".to_owned(),
                serde_json::Value::String(version.to_owned()),
            );
            Ok(value)
        }
        _ => Err(DowngradeError::Serialize(serde::ser::Error::custom(
            "internal: a versioned struct repr must serialize to a JSON object",
        ))),
    }
}
