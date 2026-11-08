//! 非 JSON 的紧凑 wire 通道。
//!
//! 设计取舍（0.3.0 调研 ⑥，实现期修正）：`$version` 标签依赖自描述
//! 格式，postcard 是非自描述的二进制格式，两者不可混用。因此 wire 层
//! **只承载当前 schema** 的紧凑字节——载荷是 final 版本明文结构的
//! postcard 编码，由 `#[version]` 宏生成的 [`WireCodec`] 实现编解码；
//! 帧头固定 8 字节：
//!
//! ```text
//! b"HIFU" | u32 LE = fnv1a32(type_name "@" version)
//! ```
//!
//! 版本哈希把 schema 钉死在帧头：发送方用旧版本序列化的帧在接收方
//! 显式报 `VersionMismatch`（携带两侧哈希与期望版本号），而不是把
//! 错 schema 的字节悄悄解成垃圾。跨版本迁移在 JSON 层完成
//! （`TryFromValue` / `to_version_value`），迁移到当前版本后再进入
//! wire 通道。
//!
//! fnv1a-32 在本模块内手写为 `const fn`，不引入依赖。

use crate::TypeMeta;
use serde::{de::DeserializeOwned, Serialize};

/// wire 帧魔数。
pub const MAGIC: [u8; 4] = *b"HIFU";

/// 帧头长度：魔数 4 字节 + 版本哈希 4 字节（小端）。
pub const HEADER_LEN: usize = 8;

/// FNV-1a 32 位哈希（const fn，零依赖）。
pub const fn fnv1a32(data: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    let mut i = 0;
    while i < data.len() {
        hash ^= data[i] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        i += 1;
    }
    hash
}

/// 计算 `type_name@version` 的版本哈希。
pub fn version_hash(type_name: &str, version: &str) -> u32 {
    let mut key = String::with_capacity(type_name.len() + version.len() + 1);
    key.push_str(type_name);
    key.push('@');
    key.push_str(version);
    fnv1a32(key.as_bytes())
}

/// 「按当前 schema 编解码」的 wire 契约，由 `#[version]` 宏为每个
/// 版本化类型生成。
///
/// 为什么不用 `Serialize`/`Deserialize` 直接做 bound：版本化类型的
/// serde 表示带 `$version` 标签（内部标签枚举），serde 对它调用
/// `deserialize_any`——postcard 这类非自描述格式直接拒绝。wire 载荷
/// 因此必须是 final 版本**明文结构**（无标签、按位序列化），只有宏
/// 能命名这个私有类型，故由宏生成本 trait 的实现。
pub trait WireCodec: Sized {
    /// 把当前 schema 的值编码为 postcard 字节。
    #[doc(hidden)]
    fn hifumi_wire_encode(&self) -> Result<Vec<u8>, WireError>;
    /// 从 postcard 字节解码为当前 schema 的值。
    #[doc(hidden)]
    fn hifumi_wire_decode(bytes: &[u8]) -> Result<Self, WireError>;
}

/// final 版本明文结构的 postcard 编码（宏生成代码的落点）。
pub fn encode_current<T: Serialize>(value: &T) -> Result<Vec<u8>, WireError> {
    postcard::to_allocvec(value).map_err(WireError::Postcard)
}

/// final 版本明文结构的 postcard 解码（宏生成代码的落点）。
pub fn decode_current<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, WireError> {
    postcard::from_bytes(bytes).map_err(WireError::Postcard)
}

/// wire 层错误。
///
/// `#[non_exhaustive]`：0.3.0 起冻结枚举形状前最后的机会——发布后再
/// 加会破坏下游的穷尽匹配。新错误形态走变体追加，不破坏既有匹配。
#[derive(Debug)]
#[non_exhaustive]
pub enum WireError {
    /// 帧长不足 8 字节或魔数不是 `b"HIFU"`。
    BadFrame,
    /// 帧头版本哈希与接收方期望不一致（发送方 schema 过旧/过新，或类型对错）。
    VersionMismatch {
        /// 帧头携带的哈希。
        wire: u32,
        /// 接收方按自身元数据计算的期望哈希。
        expected: u32,
        /// 接收方期望的 `type@version`（用于诊断输出）。
        expected_key: String,
    },
    /// 帧体 postcard 反序列化失败。
    Postcard(postcard::Error),
}

impl core::fmt::Display for WireError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            WireError::BadFrame => {
                write!(
                    f,
                    "hifumi wire: not a HIFU frame (bad magic or short header)"
                )
            }
            WireError::VersionMismatch {
                wire,
                expected,
                expected_key,
            } => write!(
                f,
                "hifumi wire: version hash mismatch (wire {wire:#010x}, expected \
                 {expected:#010x} for {expected_key}); migrate the payload through \
                 the JSON layer (TryFromValue / to_version_value) before sending \
                 it over the wire"
            ),
            WireError::Postcard(e) => write!(f, "hifumi wire: postcard payload error: {e}"),
        }
    }
}

impl std::error::Error for WireError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WireError::Postcard(e) => Some(e),
            _ => None,
        }
    }
}

impl From<postcard::Error> for WireError {
    fn from(e: postcard::Error) -> Self {
        WireError::Postcard(e)
    }
}

/// 序列化为 wire 帧：`b"HIFU" | hash LE | postcard(当前 schema)`。
///
/// `meta` 取自类型的 `__HIFUMI_META` 常量，决定帧头的版本身份。
pub fn to_wire<T: WireCodec>(value: &T, meta: &TypeMeta) -> Result<Vec<u8>, WireError> {
    let current = meta
        .versions
        .last()
        .expect("__HIFUMI_META.versions is never empty (macro always appends final)");
    let hash = version_hash(meta.type_name, current);
    let payload = value.hifumi_wire_encode()?;
    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
    frame.extend_from_slice(&MAGIC);
    frame.extend_from_slice(&hash.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

/// 从 wire 帧反序列化为 `T`（当前 schema）。
pub fn from_wire<T: WireCodec>(frame: &[u8], meta: &TypeMeta) -> Result<T, WireError> {
    if frame.len() < HEADER_LEN || frame[..4] != MAGIC {
        return Err(WireError::BadFrame);
    }
    let wire_hash = u32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]);
    let current = meta
        .versions
        .last()
        .expect("__HIFUMI_META.versions is never empty (macro always appends final)");
    let expected = version_hash(meta.type_name, current);
    if wire_hash != expected {
        return Err(WireError::VersionMismatch {
            wire: wire_hash,
            expected,
            expected_key: format!("{}@{}", meta.type_name, current),
        });
    }
    T::hifumi_wire_decode(&frame[HEADER_LEN..])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 手写 FNV-1a 与已知测试向量对拍：fnv1a32("") 与 fnv1a32("a")。
    #[test]
    fn fnv1a_known_vectors() {
        // FNV-1a 32-bit offset basis 与 "a" 的公开测试向量。
        assert_eq!(fnv1a32(b""), 0x811c_9dc5);
        assert_eq!(fnv1a32(b"a"), 0xe40c_292c);
        assert_eq!(fnv1a32(b"foobar"), 0xbf9c_f968);
    }
}
