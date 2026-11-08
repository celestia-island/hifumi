use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 宽容解析入口：`#[version]` 宏为每个版本化类型自动实现本 trait。
///
/// 已知版本（含全部历史版本，经迁移链）按正常规则解析；`$version`
/// 未知或缺失时回退为宽容读者——按当前版本表示解析整张 map，未知
/// 字段静默丢弃，缺失的当前版本必需字段仍是错误。
pub trait TryFromValue: Sized {
    /// 从 JSON 值解析本类型。
    fn try_from_value(value: &serde_json::Value) -> Result<Self, serde_json::Error>;
}

/// 前向兼容字段包装：读取时容忍未知（更新）版本的数据。
///
/// `#[version(.., forward)]` 旗标改变类型**自身**的 `Deserialize`；
/// 不方便改定义时，用 `hifumi::Forward<T>` 在字段 / 容器级别按值旁路——
/// 序列化原样透传，反序列化走 [`TryFromValue`] 的宽容回退。
///
/// ```ignore
/// #[version("0.2")]
/// struct Doc { .. }
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Envelope {
///     doc: hifumi::Forward<Doc>, // 读到 "0.9" 的 Doc 也不炸
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forward<T>(pub T);

impl<T> Forward<T> {
    /// 解包出内部值。
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: Serialize> Serialize for Forward<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T: TryFromValue> Deserialize<'de> for Forward<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        T::try_from_value(&value)
            .map_err(serde::de::Error::custom)
            .map(Forward)
    }
}
