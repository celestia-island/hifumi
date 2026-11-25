use anyhow::Result;
use hifumi::{version, Forward, TryFromValue};
use serde::{Deserialize, Serialize};

/// forward 旗标：未知（更新）版本的数据按宽容读者解析——未知字段
/// 丢弃，已知字段保留；已知旧版本照常走迁移链。
#[test]
fn forward_flag_reads_newer_payload() -> Result<()> {
    #[version("0.2", forward)]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + b: i32,
    })]
    struct Test {
        a: i32,
        b: i32,
    }

    // 旧版本照常走迁移链（Add 无转换器 → default）
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1}"#)?,
        Test { a: 1, b: 0 }
    );
    // 当前版本照常
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":1,"b":2}"#)?,
        Test { a: 1, b: 2 }
    );
    // 更新版本：未知 $version + 未知字段 b_new 丢弃，已知字段保留
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.3","a":1,"b":2,"b_new":true}"#)?,
        Test { a: 1, b: 2 }
    );
    // 缺失当前版本必需字段 → 仍是错误（宽容不等于缺字段也吞）
    assert!(serde_json::from_str::<Test>(r#"{"$version":"0.3","a":1}"#).is_err());
    // 序列化不受旗标影响：仍带当前版本标签
    assert_eq!(
        serde_json::to_string(&Test { a: 1, b: 2 })?,
        r#"{"$version":"0.2","a":1,"b":2}"#
    );

    Ok(())
}

/// forward 旗标：`$version` 完全缺失的数据也走宽容回退（按当前版本
/// 表示解析）；宽容失败（缺必需字段）时报错而不是静默捏造。
#[test]
fn forward_flag_missing_version_tag() -> Result<()> {
    #[version("0.2", forward)]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"a":1,"b":2}"#)?,
        Test { a: 1, b: 2 }
    );
    assert!(serde_json::from_str::<Test>(r#"{"a":1}"#).is_err());

    Ok(())
}

/// 无旗标类型：自身 `Deserialize` 行为不变（未知版本报错），
/// 但 `TryFromValue` 缝隙可用——同一个类型按值选择宽容策略。
#[test]
fn unflagged_type_strict_but_seam_lenient() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + b: i32,
    })]
    struct Test {
        a: i32,
        b: i32,
    }

    // 自身 Deserialize：未知版本 = unknown variant 错误（历史行为）
    assert!(serde_json::from_str::<Test>(r#"{"$version":"0.9","a":1,"b":2}"#).is_err());
    // TryFromValue 缝隙：宽容
    let value = serde_json::json!({ "$version": "0.9", "a": 1, "b": 2, "x": 0 });
    assert_eq!(
        <Test as TryFromValue>::try_from_value(&value)?,
        Test { a: 1, b: 2 }
    );
    // 缝隙同样支持旧版本迁移
    let old = serde_json::json!({ "$version": "0.1", "a": 5 });
    assert_eq!(
        <Test as TryFromValue>::try_from_value(&old)?,
        Test { a: 5, b: 0 }
    );

    Ok(())
}

/// `Forward<T>`：字段级旁路。Envelope 的 doc 字段读新数据不炸，
/// 序列化原样透传，旧版本照常迁移。
#[test]
fn forward_wrapper_envelope() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + b: i32,
    })]
    struct Doc {
        a: i32,
        b: i32,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Envelope {
        doc: Forward<Doc>,
        note: String,
    }

    // 更新版本的 Doc：宽容读入 Envelope
    let env: Envelope =
        serde_json::from_str(r#"{"doc":{"$version":"0.9","a":1,"b":2,"future":3},"note":"n"}"#)?;
    assert_eq!(env.note, "n");
    assert_eq!(env.doc.into_inner(), Doc { a: 1, b: 2 });

    // 旧版本 Doc 照常走迁移链
    let env: Envelope = serde_json::from_str(r#"{"doc":{"$version":"0.1","a":7},"note":"m"}"#)?;
    assert_eq!(env.doc.into_inner(), Doc { a: 7, b: 0 });

    // 序列化透传：Doc 自带 $version 标签，Envelope 不加第二层
    let env = Envelope {
        doc: Forward(Doc { a: 1, b: 2 }),
        note: "n".to_owned(),
    };
    assert_eq!(
        serde_json::to_string(&env)?,
        r#"{"doc":{"$version":"0.2","a":1,"b":2},"note":"n"}"#
    );

    Ok(())
}

/// 元数据：`__HIFUMI_META` 的版本链顺序（旧 → 新）、可逆性与旗标。
#[test]
fn meta_reflects_chain() {
    #[version("0.2", forward)]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + b: i32,
    })]
    struct AddOnly {
        a: i32,
        b: i32,
    }

    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        c: u8 => d: i32 { i32::from(c) },
    })]
    struct Lossy {
        d: i32,
    }

    assert_eq!(AddOnly::__HIFUMI_META.type_name, "AddOnly");
    assert_eq!(AddOnly::__HIFUMI_META.versions, &["0.1", "0.2"]);
    const { assert!(AddOnly::__HIFUMI_META.reversible) }
    const { assert!(AddOnly::__HIFUMI_META.forward) }

    const { assert!(!Lossy::__HIFUMI_META.reversible) }
    const { assert!(!Lossy::__HIFUMI_META.forward) }
}

/// 元数据：无迁移的类型版本链只有当前版本，链为空 → 恒可逆。
#[test]
fn meta_single_version() {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    struct Solo {
        a: i32,
    }

    assert_eq!(Solo::__HIFUMI_META.versions, &["0.2"]);
    const { assert!(Solo::__HIFUMI_META.reversible) }
}

/// 裸旗标：`#[version(forward)]` 版本字面量缺省回退 CARGO_PKG_VERSION，
/// 旗标照常生效（解析器两个维度正交）。
#[test]
fn forward_flag_without_version_uses_pkg_version() {
    #[version(forward)]
    #[derive(Debug, Clone, PartialEq)]
    struct EnvDoc {
        a: i32,
    }

    assert_eq!(EnvDoc::__HIFUMI_META.versions, &[env!("CARGO_PKG_VERSION")]);
    const { assert!(EnvDoc::__HIFUMI_META.forward) }

    let parsed: EnvDoc =
        serde_json::from_str(r#"{"$version":"9.9","a":7,"future_field":true}"#).unwrap();
    assert_eq!(parsed.a, 7);
}

/// 三层链的版本链顺序（多步回溯后仍保持旧 → 新）。
#[test]
fn meta_three_step_chain() {
    #[version("0.3")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.2" => "0.3" {
        + c: i32,
    })]
    #[migration("0.1" => "0.2" {
        + b: i32,
    })]
    struct Chain {
        a: i32,
        b: i32,
        c: i32,
    }

    assert_eq!(Chain::__HIFUMI_META.versions, &["0.1", "0.2", "0.3"]);
    const { assert!(Chain::__HIFUMI_META.reversible) }
}

/// `forward` 的可观察边界——一张表钉死。机制上回退对**任何**版本化解码失败
/// 都触发（生成的 `Deserialize` 里是 `Err(_)`），再按当前版本重读；但从外面
/// 看，它与「只有版本定不下来才回退」不可区分：一个声明了已知旧版本、却不符
/// 那个版本形状的载荷，落到当前版本形状上后要求的只会更多，于是以同样的
/// 方式失败。所以这张表里 forward 与 strict 只在「无 tag」和「未知 tag」两行
/// 不同。若将来有人把 `Err(_)` 收窄到「仅 tag 不在枚举里」，这张表**不会**变红
/// ——那正是「不可区分」的含义；要让那种收窄可见，得先有一个当前版本比旧版本
/// 要求更少的类型，而 `+ field` 迁移语法造不出那种类型。
#[test]
fn forward_observable_boundary_is_only_the_unresolvable_tag() -> Result<()> {
    #[version("0.2", forward)]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + b: String,
    })]
    struct Fwd {
        a: i32,
        b: String,
    }
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + b: String,
    })]
    struct Strict {
        a: i32,
        b: String,
    }
    type Verdict = Result<(), String>;
    let run = |body: &str| -> (Verdict, Verdict) {
        let v: serde_json::Value = serde_json::from_str(body).expect("probe json");
        let f = serde_json::from_value::<Fwd>(v.clone())
            .map(|_| ())
            .map_err(|e| e.to_string());
        let s = serde_json::from_value::<Strict>(v)
            .map(|_| ())
            .map_err(|e| e.to_string());
        (f, s)
    };
    //           payload                           forward  strict   why
    for (body, fwd_ok, strict_ok, why) in [
        (r#"{"$version":"0.1","a":1}"#,             true,  true,  "known older version: the migration chain supplies the added field"),
        (r#"{"$version":"0.1","a":"x","b":"y"}"#,    false, false, "known version, wrong-typed known field: refused both ways"),
        (r#"{"$version":"0.1","b":"y"}"#,            false, false, "known version, missing its own required field: the fallback re-reads as the current version, which also requires it"),
        (r#"{"$version":"0.2","a":"x","b":"y"}"#,    false, false, "current version, wrong-typed field: refused both ways"),
        (r#"{"a":1,"b":"y"}"#,                       true,  false, "NO tag: forward re-reads leniently; strict reports a missing `$version`"),
        (r#"{"$version":"9.9","a":1,"b":"y"}"#,      true,  false, "UNKNOWN tag: forward re-reads leniently; strict reports an unknown variant"),
        (r#"{"$version":"9.9","a":"x","b":"y"}"#,    false, false, "unknown tag AND a wrong-typed known field: the lenient reader still type-checks what it knows"),
    ] {
        let (f, s) = run(body);
        assert_eq!(f.is_ok(), fwd_ok, "forward  ← {body}  ({why}): {f:?}");
        assert_eq!(s.is_ok(), strict_ok, "strict   ← {body}  ({why}): {s:?}");
    }
    Ok(())
}
