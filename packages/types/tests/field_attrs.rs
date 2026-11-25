//! 字段级 serde 属性透传。
//!
//! `#[version]` 宏自己生成 `Serialize`/`Deserialize`，用户因此不能再
//! `#[derive(Serialize)]`（E0119），字段上的 `#[serde(...)]` 也无人注册——
//! rustc 报 `cannot find attribute 'serde' in this scope`，**整份文件无法
//! 编译**。因此**凡是用 `#[serde(...)]` 的测试**都是「缺了透传就不编译」的
//! 翡翠线；语义断言之外还钉住 `$version` 标签位置与 wire 帧头哈希，证明透传
//! 没有动表示。`#[doc]` 不同：它在白名单里但只作注释，去掉透传它仍能编译，
//! 见 `doc_attr_passes_through_without_invalid_serde`。

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

/// 非 serde 的白名单属性（这里：`#[doc]`）不破坏编译，且不影响表示：
/// 它被搬运到隐藏结构体上只是注释，不参与编解码。
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

/// token 扫描只认 `Ident`：字段名/键名里出现 `flatten` 这个**字符串**时不得误拒。
/// （`"flatten"` 是 `Literal` 而不是 `Ident`，所以扫描不会命中。）
#[test]
fn flatten_as_a_string_is_not_a_rejection() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    struct Conf {
        #[serde(rename = "flatten")]
        verbose: i32,
    }

    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.2","flatten":7}"#)?,
        Conf { verbose: 7 }
    );

    Ok(())
}

/// `#[serde(alias)]` 与 `rename` 走同一条转发路径：字段名与类型未变的历史结构体
/// 同样带上它。这是 README 推荐「改名后让旧载荷继续可读」的前提——若哪天只有
/// 当前版本认 `alias`，旧键载荷会重新解析失败，而 README 的说法就成了空话。
#[test]
fn serde_alias_rides_the_same_forwarding_as_rename() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2")]
    struct Conf {
        #[serde(rename = "id", alias = "a")]
        a: i32,
        #[serde(default)]
        b: i32,
    }

    // 旧键（改名前写出的载荷）：仍能读。
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.1","a":1}"#)?,
        Conf { a: 1, b: 0 }
    );
    // 新键：走迁移链，照常可读。
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.1","id":2}"#)?,
        Conf { a: 2, b: 0 }
    );
    // 当前版本两个键都认。
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.2","a":3,"b":4}"#)?,
        Conf { a: 3, b: 4 }
    );

    Ok(())
}

/// 回归钉：属性不跟着**名字**走，只跟着**字段身份**走。
///
/// 这条链里 `n` 这个名字被复用了一次：
/// `0.1 => 0.2 { n: i32 => m: i32 }` 把 0.1 的 `n` 改名成 `m`，
/// 随后 `0.2 => 0.3 { + n: i32 }` 又加了一个**全新的** `n`。
/// 于是 0.1 的 `n` 与 0.3 的 `n` 同名、同型，却是两个毫不相干的字段。
///
/// 若按「同名同型」搬 `#[serde(default)]`，0.1 一个本来必需的字段就变成
/// 可缺省：`{"$version":"0.1","m":9}` 会被静默读成 `m:0, n:0`（那个 9
/// 直接丢掉），损坏数据当成新数据迁上去。必须保持「0.1 缺 `n` = 解析失败」。
#[test]
fn attribute_does_not_follow_a_reused_name() -> Result<()> {
    #[version("0.3")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.2" => "0.3" { + n: i32 })]
    #[migration("0.1" => "0.2" { n: i32 => m: i32 })]
    struct Doc {
        m: i32,
        #[serde(default)]
        n: i32,
    }

    // 0.1 的 `n` 是必需字段（它是被改名成 m 的那个），载荷缺它必须报错。
    let missing = serde_json::from_str::<Doc>(r#"{"$version":"0.1"}"#);
    assert!(missing.is_err(), "0.1 missing `n` must fail: {missing:?}");

    // 同理：只给 `m` 不给 `n`，那个 9 不许被静默吞掉。
    let partial = serde_json::from_str::<Doc>(r#"{"$version":"0.1","m":9}"#);
    assert!(
        partial.is_err(),
        "0.1 payload without `n` must not silently truncate data: {partial:?}"
    );

    // 0.1 的 `n` 在场时照常迁移：n=5 -> m=5（改名），0.3 的 n 由迁移补 0。
    assert_eq!(
        serde_json::from_str::<Doc>(r#"{"$version":"0.1","n":5}"#)?,
        Doc { m: 5, n: 0 }
    );

    // 当前版本缺 `n`：由 `#[serde(default)]` 补——属性仍留在它自己的字段上。
    assert_eq!(
        serde_json::from_str::<Doc>(r#"{"$version":"0.3","m":1}"#)?,
        Doc { m: 1, n: 0 }
    );

    Ok(())
}

/// 对照组：字段真的原样穿过一条多跳链时，属性**必须**继续继承。
///
/// 与 `attribute_does_not_follow_a_reused_name` 配套——只钉「不搬」会让
/// 「干脆全都不搬」也通过，那等于把功能关掉。
#[test]
fn attribute_follows_a_field_through_a_multi_hop_chain() -> Result<()> {
    #[version("0.3")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.2" => "0.3" { + extra: i32 })]
    #[migration("0.1" => "0.2" { + also: i32 })]
    struct Keep {
        a: i32,
        #[serde(default)]
        tail: i32,
        extra: i32,
        also: i32,
    }

    // `tail` 在两跳里都原样存在：0.1 侧缺它仍能被 default 补上。
    assert_eq!(
        serde_json::from_str::<Keep>(r#"{"$version":"0.1","a":1}"#)?,
        Keep {
            a: 1,
            tail: 0,
            extra: 0,
            also: 0
        }
    );

    Ok(())
}

/// `Add` 无转换器时，缺省值必须跟着字段自己的 `#[serde(default = "path")]` 走。
///
/// 否则同一个字段在边界两侧取不同的缺省值：0.2 载荷走 serde（`seven()` → 7），
/// 0.1 载荷走迁移链（`Default::default()` → 0）。同一份数据换个 `$version`
/// 就得到不同结果，这是最难查的那类不一致。
#[test]
fn added_field_uses_its_serde_default_function_on_both_sides() -> Result<()> {
    fn seven() -> i32 {
        7
    }

    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" { + n: i32 })]
    struct Conf {
        a: i32,
        #[serde(default = "seven")]
        n: i32,
    }

    // 当前版本缺 n：serde 用 seven()
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.2","a":1}"#)?,
        Conf { a: 1, n: 7 }
    );
    // 旧版本缺 n：迁移**必须**用同一个 seven()，不是 Default::default()
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.1","a":1}"#)?,
        Conf { a: 1, n: 7 }
    );

    Ok(())
}

/// 同上，但用一个**没有实现 `Default`** 的类型：`NonZeroU32`。
///
/// 这种字段只有 `#[serde(default = "path")]` 这一条路可写——若迁移侧坚持
/// `<T as Default>::default()`，整份代码编译不过（`E0277: NonZero<u32>:
/// Default is not satisfied`），尽管缺省值已经完全指定好了。
#[test]
fn added_non_default_type_compiles_via_its_default_function() -> Result<()> {
    use std::num::NonZeroU32;

    fn one() -> NonZeroU32 {
        NonZeroU32::new(1).expect("1 is non-zero")
    }

    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" { + id: NonZeroU32 })]
    struct Conf {
        a: i32,
        #[serde(default = "one")]
        id: NonZeroU32,
    }

    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.1","a":1}"#)?,
        Conf { a: 1, id: one() }
    );
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.2","a":1}"#)?,
        Conf { a: 1, id: one() }
    );

    Ok(())
}

/// 裸 `#[serde(default)]`（无函数）仍等价于 `Default::default()`——
/// 两条路径保持一致，这是 `added_field_uses_its_serde_default_function_*`
/// 的对照组：改了上面的路径不能把这一条也改歪。
#[test]
fn bare_serde_default_still_matches_default_default() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" { + n: i32 })]
    struct Conf {
        a: i32,
        #[serde(default)]
        n: i32,
    }

    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.1","a":1}"#)?,
        Conf { a: 1, n: 0 }
    );
    assert_eq!(
        serde_json::from_str::<Conf>(r#"{"$version":"0.2","a":1}"#)?,
        Conf { a: 1, n: 0 }
    );

    Ok(())
}
