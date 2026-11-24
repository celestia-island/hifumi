//! 字段级 serde 属性透传。
//!
//! `#[version]` 宏自己生成 `Serialize`/`Deserialize`，用户因此不能再
//! `#[derive(Serialize)]`（E0119），字段上的 `#[serde(...)]` 也无人注册——
//! rustc 报 `cannot find attribute 'serde' in this scope`，**整份文件无法
//! 编译**。这里的每个测试都是「缺了透传就不编译」的翡翠线：语义断言之外
//! 还钉住 `$version` 标签位置与 wire 帧头哈希，证明透传没有动表示。

use anyhow::Result;
use hifumi::{version, wire};

/// 字段缺省即取 `Default`：载荷里少该字段也能解析（evernight 的解锁点）。
#[test]
fn serde_default_fills_missing_field() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    struct Conf {
        a: i32,
        #[serde(default)]
        b: i32,
    }

    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.2","a":1}"#)?,
        Conf { a: 1, b: 0 }
    );
    // 字段在场时行为不变
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.2","a":1,"b":7}"#)?,
        Conf { a: 1, b: 7 }
    );

    Ok(())
}

/// `#[serde(default)]` 同时生效在**隐藏的旧版本结构体**上：迁移链解析
/// 0.1 载荷时旧结构体本身就接受缺字段（这条以前只会因为「属性没搬过去」
/// 而落在当前结构体上，旧结构体仍是硬要求）。
#[test]
fn serde_default_applies_to_hidden_old_structs() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2")]
    struct Conf {
        a: i32,
        #[serde(default)]
        b: i32,
    }

    // 0.1 的结构体只有 a，b 在该版本里还不存在；但即便载荷里带了一个
    // 额外字段也不该报错——旧侧字段的 default 属性是从当前侧按名字搬过去的。
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.1","a":1}"#)?,
        Conf { a: 1, b: 0 }
    );
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.2","a":1}"#)?,
        Conf { a: 1, b: 0 }
    );

    Ok(())
}

/// `#[serde(rename)]` 字段键：透传后旧版本结构体与当前结构体用同一个
/// JSON 键，迁移照常（0.1 侧缺 `b` 由 `#[serde(default)]` 补齐）。
#[test]
fn serde_rename_field_key() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2")]
    struct Conf {
        #[serde(rename = "id")]
        a: i32,
        #[serde(default)]
        b: i32,
    }

    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.1","id":1}"#)?,
        Conf { a: 1, b: 0 }
    );
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.2","id":1,"b":2}"#)?,
        Conf { a: 1, b: 2 }
    );

    Ok(())
}

/// `#[serde(skip_serializing_if)]` 字段属性：写在（隐藏的）final 明文
/// 结构体上，序列化时的省略行为直接体现在 `$version` 文档里。
#[test]
fn serde_skip_serializing_if_is_honoured() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    struct Conf {
        a: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    }

    assert_eq!(
        serde_json::to_string(&Conf { a: 1, note: None })?,
        r#"{"$version":"0.2","a":1}"#
    );
    assert_eq!(
        serde_json::to_string(&Conf {
            a: 1,
            note: Some("x".to_string())
        })?,
        r#"{"$version":"0.2","a":1,"note":"x"}"#
    );

    Ok(())
}

/// 表示不变性证明：加了透传属性的类型，其字段齐备载荷的 JSON 与
/// 「没有这些属性」的类型逐字节相同——`$version` 仍由内部标签枚举写在
/// 载荷最前面，且 wire 帧头哈希仍是 `type@version` 的 FNV-1a。
#[test]
fn passthrough_keeps_version_tag_and_wire_hash() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    struct Plain {
        a: i32,
        b: i32,
    }

    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    struct WithAttrs {
        a: i32,
        #[serde(default)]
        #[serde(alias = "old_b")]
        b: i32,
    }

    let plain = serde_json::to_string(&Plain { a: 1, b: 2 })?;
    let with_attrs = serde_json::to_string(&WithAttrs { a: 1, b: 2 })?;
    assert_eq!(plain, with_attrs);
    assert_eq!(with_attrs, r#"{"$version":"0.2","a":1,"b":2}"#);

    // 别名只影响读取，不影响写出的键
    assert_eq!(
        serde_json::from_str::<WithAttrs>(r#"{"$version":"0.2","a":1,"old_b":2}"#)?,
        WithAttrs { a: 1, b: 2 }
    );

    // wire 帧头哈希 = FNV-1a("WithAttrs@0.2")：身份仍由类型名与版本串决定，
    // 字段属性不参与。
    let frame = wire::to_wire(&WithAttrs { a: 1, b: 2 }, &WithAttrs::__HIFUMI_META)?;
    let hash = u32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]);
    assert_eq!(hash, wire::version_hash("WithAttrs", "0.2"));

    Ok(())
}

/// 白名单外的属性（这里：`#[doc]`）不破坏编译，且不影响表示。
#[test]
fn doc_attr_passes_through_without_invalid_serde() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    struct Conf {
        /// 字段文档在生成物上照抄一份。
        a: i32,
        b: i32,
    }

    assert_eq!(
        serde_json::to_string(&Conf { a: 1, b: 2 })?,
        r#"{"$version":"0.2","a":1,"b":2}"#
    );

    Ok(())
}
