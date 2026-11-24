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

/// `#[serde(default)]` 只落在真正的那个字段上：它补出来的缺口是「当前
/// 版本缺字段」，不是「旧版本缺字段」。
///
/// `b` 由 `Add` 规则引入，0.1 版本里根本不存在——旧结构体上不该有它，
/// 属性也就不该搬过去。两条路径都必须能解析。
#[test]
fn serde_default_on_added_field() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2")]
    struct Conf {
        a: i32,
        #[serde(default)]
        b: i32,
    }

    // 0.1 载荷：b 在那一版还不存在，走迁移链补默认值
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.1","a":1}"#)?,
        Conf { a: 1, b: 0 }
    );
    // 0.2 载荷缺 b：由 #[serde(default)] 补
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.2","a":1}"#)?,
        Conf { a: 1, b: 0 }
    );

    Ok(())
}

/// 回归钉：属性**不**许跨类型搬运。
///
/// `a` 在 0.1 里是 `i32`、在 0.2 里是 `String`——同名不同型，是两个字段。
/// 若按名字无条件把 `#[serde(default)]` 搬给 0.1 的 `a`，旧版本一个本来
/// **必需**的字段会变成可缺省：损坏的旧数据被静默补成 0 再迁成 `"0"`。
/// 必须保持「旧版本缺该字段 = 解析失败」。
#[test]
fn serde_default_does_not_leak_across_a_type_change() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        a: i32 => a: String { a.to_string() },
    })]
    struct Conf {
        #[serde(default)]
        a: String,
    }

    // 当前版本：缺字段按 default 补齐
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.2"}"#)?,
        Conf { a: String::new() },
    );
    // 旧版本：a 是必需字段，缺失必须仍然报错（不能被 default 吞掉）
    assert!(serde_json::from_str::<Conf>(r#"{"$version":"0.1"}"#).is_err());
    // 旧版本：a 在场则照常迁移
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.1","a":42}"#)?,
        Conf {
            a: "42".to_string()
        }
    );

    Ok(())
}

/// `#[serde(rename)]` 字段键：同型字段在旧版本结构体上沿用同一个 JSON
/// 键，迁移照常（0.1 侧缺 `b` 由同字段的 `#[serde(default)]` 补齐）。
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

    // wire 帧逐字节钉死：魔数 + `type@version` 的 FNV-1a + postcard 载荷。
    // 期望值全写成字面量，而不是从 `__HIFUMI_META` 反算——反算的断言恒真，
    // 什么也证明不了。尾巴 `203, 236, 186, 231` LE = 3887787211 =
    // fnv1a("WithAttrs@0.2")；`2, 4` 是 postcard 下 a=1、b=2 的 zigzag 编码。
    let frame = wire::to_wire(&WithAttrs { a: 1, b: 2 }, &WithAttrs::__HIFUMI_META)?;
    assert_eq!(
        frame,
        [b'H', b'I', b'F', b'U', 203, 236, 186, 231, 2, 4],
        "attribute-carrying type must keep the pre-existing frame bytes"
    );

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

/// 降级写回同样 honour 字段属性：`to_version_value` 序列化的是隐藏的旧
/// 版本明文结构体，属性跟着它走，`$version` 仍由组装助手注入在最前面。
#[test]
fn passthrough_applies_to_downgrade_writes() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2")]
    struct Conf {
        a: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    }

    let current = Conf { a: 1, note: None };
    let older = current.to_version_value("0.1").unwrap();
    // 0.1 的表示里 note 被 skip_serializing_if 省掉，$version 仍排在第一位
    assert_eq!(older, serde_json::json!({ "$version": "0.1", "a": 1 }));

    let current = Conf {
        a: 1,
        note: Some("x".to_string()),
    };
    let older = current.to_version_value("0.1").unwrap();
    assert_eq!(
        older,
        serde_json::json!({ "$version": "0.1", "a": 1, "note": "x" })
    );

    Ok(())
}
