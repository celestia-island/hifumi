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

/// `forward` 的可观察边界（只加字段的链）——一张表钉死。机制上回退对**任何**
/// 版本化解码失败都触发（生成的 `Deserialize` 里是 `Err(_)`），再按当前版本重读。
/// 对**只加字段**的链，当前版本的要求是每个旧版本要求的超集：一个声明了已知旧
/// 版本、却不符那个版本形状的载荷（缺字段或字段类型错），落到当前版本形状后
/// 校验的是同一份载荷，让已知版本失败的那个字段在那儿依然缺失或依然类型错——
/// 所以以同样的方式失败，与「只有版本定不下来才回退」不可区分。因此这张表里
/// forward 与 strict 只在「无 tag」和「未知 tag」两行不同。
///
/// comment-gate-ok: 这不是生产行为的断言，而是**这张测试表**的覆盖说明——
/// 它由 MUT:add-only-table 验证（把 `Err(_)` 收窄成「仅 tag 不在枚举里」后，
/// 本文件 6 条 forward 测试仍全绿，正是「只加字段时不可区分」的定义）。
///
/// 等价性破裂的**精确条件**：某步之后，存在一个**符合当前版本形状却不符合旧版本
/// 形状**的旧载荷——用「要求集合的超集关系」去刻画它不可靠（按值集读，放宽重定型
/// `n: u8 => n: i32` 保持超集关系却仍宽化：`{"n":300}` 符合新形状不符合旧形状）。
/// `Remove`、`Rename`（无论目标是否必填）、以及放宽重定型都会造出这样的载荷
/// ——见下方两个 `forward_widens_*` 测试；收窄重定型（`n: i32 => n: u8`）造不出。
/// 若将来有人把 `Err(_)` 收窄到「仅 tag 不在枚举里」，**这张表**不会变红；那正
/// 是「只加字段时不可区分」的含义。
#[test]
fn forward_add_only_boundary_moves_only_on_an_unresolvable_tag() -> Result<()> {
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

/// 迁移步骤是 **`Remove`** 时，边界确实移动：当前版本比旧版本少要一个字段，
/// 于是「只缺那个被删字段」的旧版本载荷在 forward 下被接受（回落按当前版本重读，
/// 不再要求 `c`），strict 下被拒绝（0.1 变体要求 `c`）。这是 forward 可观察宽化的
/// **最清楚形态**；破裂的精确条件是「存在符合当前版本形状却不符合旧版本形状的旧
/// 载荷」，`Rename` 与放宽重定型也能造出（见下一张表）。README 的 forward 注释据此措辞。
#[test]
fn forward_widens_observably_after_a_remove_step() -> Result<()> {
    #[version("0.2", forward)]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        - c: i32,
    })]
    struct F {
        a: i32,
    }
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        - c: i32,
    })]
    struct S {
        a: i32,
    }
    let run = |body: &str| -> (bool, bool) {
        let v: serde_json::Value = serde_json::from_str(body).expect("probe json");
        let f = serde_json::from_value::<F>(v.clone()).is_ok();
        let s = serde_json::from_value::<S>(v).is_ok();
        (f, s)
    };
    //         payload                       forward  strict  why
    for (body, fwd_ok, strict_ok, why) in [
        (r#"{"$version":"0.1","a":1,"c":9}"#, true,  true,  "complete old payload: the step drops c; both decode"),
        // THE discriminating row — the one the README's claim turns on:
        (r#"{"$version":"0.1","a":1}"#,     true,  false, "old payload missing the REMOVED field: forward re-reads as the current version, which no longer requires c; strict's 0.1 variant still does"),
        (r#"{"$version":"0.1","c":9}"#,     false, false, "missing a, which BOTH versions require"),
    ] {
        let (f, s) = run(body);
        assert_eq!(f, fwd_ok, "forward  ← {body}  ({why})");
        assert_eq!(s, strict_ok, "strict   ← {body}  ({why})");
    }
    Ok(())
}
/// 等价性破裂的另外两种形态——`Rename`（即便是到**必填**目标）与同名字段重定型。
/// 共同点：一个**已符合当前版本形状、却不符合旧版本形状**的旧载荷，在 forward
/// 下被接受、不带则被拒绝。「缺所有字段」两边都 Err（当前版本仍要它的字段）。
/// 这纠正了一个直觉错误：是否宽化与目标字段是否必填**无关**，只取决于「载荷符
/// 合新形状但不符合旧形状」这一关系能否出现。
#[test]
fn forward_widens_across_rename_and_widening_retype_steps() -> Result<()> {
    #[version("0.2", forward)]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        c: u8 => d: i32 { i32::from(c) },
    })]
    struct FRenamed {
        d: i32,
    }
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        c: u8 => d: i32 { i32::from(c) },
    })]
    struct SRenamed {
        d: i32,
    }
    #[version("0.2", forward)]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        n: i32 => n: String { n.to_string() },
    })]
    struct FRetyped {
        n: String,
    }
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        n: i32 => n: String { n.to_string() },
    })]
    struct SRetyped {
        n: String,
    }
    let run = |body: &str| -> (bool, bool, bool, bool) {
        let v: serde_json::Value = serde_json::from_str(body).expect("probe json");
        (
            serde_json::from_value::<FRenamed>(v.clone()).is_ok(),
            serde_json::from_value::<SRenamed>(v.clone()).is_ok(),
            serde_json::from_value::<FRetyped>(v.clone()).is_ok(),
            serde_json::from_value::<SRetyped>(v).is_ok(),
        )
    };
    //         payload                   renamed fwd/strict  retyped fwd/strict  why
    for (body, rf, rs, tf, ts, why) in [
        (r#"{"$version":"0.1","c":5}"#,   true,  true,  false, false, "the OLD shape: the step migrates it (renamed) / retype sees no n (both sides)"),
        (r#"{"$version":"0.1","d":5}"#,   true,  false, false, false, "already the NEW shape, missing the old name: forward re-reads as current; strict's 0.1 wants c"),
        (r#"{"$version":"0.1"}"#,         false, false, false, false, "nothing: the current version still requires its field"),
        (r#"{"$version":"0.1","n":"7"}"#,  false, false, true,  false, "already the NEW (String) shape: forward re-reads; strict's 0.1 wants an i32 n"),
    ] {
        let (rf_, rs_, tf_, ts_) = run(body);
        assert_eq!((rf_, rs_, tf_, ts_), (rf, rs, tf, ts), "{body}  ({why})");
    }
    Ok(())
}
