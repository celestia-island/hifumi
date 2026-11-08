//! ⑤ 链压缩（baseline）：灭绝版本不再生成，读取/降级到灭绝版本
//! 显式报错，链从 baseline 结构起算。

use anyhow::Result;
use hifumi::{version, DowngradeError};

/// 基线截断：0.1 灭绝，0.2 仍可读可降；meta 如实反映压缩事实。
#[test]
fn baseline_truncates_chain() -> Result<()> {
    #[version("0.3", baseline = "0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + extra: i32,
    })]
    #[migration("0.2" => "0.3" {
        + note: String,
    })]
    struct Doc {
        extra: i32,
        note: String,
    }

    // ① meta：链只含 0.2 → 0.3，baseline 如实记录
    assert_eq!(Doc::__HIFUMI_META.versions, &["0.2", "0.3"]);
    assert_eq!(Doc::__HIFUMI_META.baseline, Some("0.2"));

    // ② 读 0.2（基线版本）照常迁移到当前
    let v02 = serde_json::json!({
        "$version": "0.2",
        "extra": 7,
    });
    let doc: Doc = serde_json::from_value(v02)?;
    assert_eq!(
        doc,
        Doc {
            extra: 7,
            note: String::new(),
        }
    );

    // ③ 读 0.1（灭绝版本）= unknown variant：旧结构根本没生成
    let v01 = serde_json::json!({
        "$version": "0.1",
        "extra": 7,
    });
    let err = serde_json::from_value::<Doc>(v01).unwrap_err();
    assert!(
        err.to_string().contains("unknown variant"),
        "extinct version must fail as unknown variant, got: {err}"
    );

    // ④ 降级到 0.2 可达（该步免费可逆 + 有 back 缺省），降级到 0.1 不可达
    let down = doc.to_version_value("0.2")?;
    assert_eq!(down.get("$version"), Some(&serde_json::json!("0.2")));
    match doc.clone().to_version_value("0.1") {
        Err(DowngradeError::UnknownTarget { target, .. }) => assert_eq!(target, "0.1"),
        other => panic!("downgrade to extinct version must be UnknownTarget, got {other:?}"),
    }

    // ⑤ 无 baseline 的同形链版本列表齐全（负对照：未压缩行为不变）
    #[version("0.3")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + extra: i32,
    })]
    #[migration("0.2" => "0.3" {
        + note: String,
    })]
    struct Full {
        extra: i32,
        note: String,
    }
    assert_eq!(Full::__HIFUMI_META.versions, &["0.1", "0.2", "0.3"]);
    assert_eq!(Full::__HIFUMI_META.baseline, None);
    let _: Full = serde_json::from_value(serde_json::json!({
        "$version": "0.1",
        "extra": 7,
    }))?;

    Ok(())
}

/// baseline = 当前版本：整条链灭绝，只剩最终表示。
#[test]
fn baseline_at_current_keeps_only_final() -> Result<()> {
    #[version("0.2", baseline = "0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + extra: i32,
    })]
    struct Only {
        extra: i32,
    }

    assert_eq!(Only::__HIFUMI_META.versions, &["0.2"]);
    assert_eq!(Only::__HIFUMI_META.baseline, Some("0.2"));

    let doc: Only = serde_json::from_value(serde_json::json!({
        "$version": "0.2",
        "extra": 3,
    }))?;
    assert_eq!(doc.extra, 3);

    // 旧版本数据 = unknown variant
    let err = serde_json::from_value::<Only>(serde_json::json!({
        "$version": "0.1",
        "extra": 3,
    }))
    .unwrap_err();
    assert!(err.to_string().contains("unknown variant"));

    // 甚至无法降级回 0.1（链上只有 0.2）
    match doc.to_version_value("0.1") {
        Err(DowngradeError::UnknownTarget { target, .. }) => assert_eq!(target, "0.1"),
        other => panic!("expected UnknownTarget, got {other:?}"),
    }

    Ok(())
}

/// baseline + forward 组合：灭绝（更老）版本走 forward 的宽容回退路径
/// ——按最终表示解析。这是两个旗标组合的诚实语义：forward 承诺的是
/// 「未知 `$version` 按宽容读者处理」，不区分未知来自未来还是过去。
#[test]
fn baseline_with_forward_lenient_on_extinct() -> Result<()> {
    #[version("0.3", forward, baseline = "0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + extra: i32,
    })]
    #[migration("0.2" => "0.3" {
        + note: String,
    })]
    struct Lenient {
        extra: i32,
        note: String,
    }

    // 灭绝版本 0.1 的负载按最终表示宽容解析（字段兼容即成功）
    let doc: Lenient = serde_json::from_value(serde_json::json!({
        "$version": "0.1",
        "extra": 9,
        "note": "kept",
    }))?;
    assert_eq!(
        doc,
        Lenient {
            extra: 9,
            note: "kept".to_string(),
        }
    );

    // 基线版本 0.2 仍走真实迁移路径（Add 步给了 note 默认空串）
    let doc2: Lenient = serde_json::from_value(serde_json::json!({
        "$version": "0.2",
        "extra": 4,
    }))?;
    assert_eq!(doc2.note, "");

    Ok(())
}

/// 注册表把压缩事实带进 spec()：baseline 字段与截断后的版本链
/// 都是工具链（如 TS runner 导出）可见的静态事实。
mod registry_baseline {
    use hifumi::version;

    #[version("0.3", baseline = "0.1")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + extra: i32,
    })]
    #[migration("0.2" => "0.3" {
        + note: String,
    })]
    struct Reg {
        extra: i32,
        note: String,
    }

    hifumi::registry! {
        policy: backward {
            Reg,
        }
    }
}

#[test]
fn baseline_visible_in_registry_spec() {
    let spec = registry_baseline::spec();
    assert_eq!(spec.len(), 1);
    assert_eq!(spec[0].type_name, "Reg");
    // 链只含 0.1 → 0.3（0.1 之上两步保留），baseline 记为 0.1
    assert_eq!(spec[0].versions, &["0.1", "0.2", "0.3"]);
    assert_eq!(spec[0].baseline, Some("0.1"));
}
