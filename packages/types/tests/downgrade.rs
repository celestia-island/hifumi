use anyhow::Result;
use hifumi::{version, DowngradeError};

/// 免费可逆操作（加字段 / 复制 / 纯改名）的链：降级 + 重新升级回到
/// 0.1 可表达的值（Add 字段按设计有损回默认值；Copy 字段从旧源重建）。
#[test]
fn free_ops_roundtrip() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + extra: i32,
        + a => d: i32,
        b => e: i32,
    })]
    struct Test {
        a: i32,
        d: i32,
        e: i32,
        extra: i32,
    }

    let current = Test {
        a: 1,
        d: 7,
        e: 2,
        extra: 5,
    };

    let downgraded = current.to_version_value("0.1")?;
    assert_eq!(
        downgraded,
        serde_json::json!({
            "$version": "0.1",
            "a": 1,
            "b": 2,
        })
    );

    // 降级产物能被正向反序列化吃回去；Add 回默认值，Copy 从旧 a 重建
    let back = serde_json::from_value::<Test>(downgraded)?;
    assert_eq!(
        back,
        Test {
            a: 1,
            d: 1,
            e: 2,
            extra: 0
        }
    );

    // 当前版本 = 普通序列化
    let now = current.to_version_value("0.2")?;
    assert_eq!(
        now,
        serde_json::json!({
            "$version": "0.2",
            "a": 1,
            "d": 7,
            "e": 2,
            "extra": 5,
        })
    );

    Ok(())
}

/// 带变换的 Remove 声明 `back` 后可逆。
#[test]
fn remove_with_back_roundtrip() -> Result<()> {
    #[version("0.3")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.2" => "0.3" {
        - legacy: bool { back: level < 2 },
    })]
    struct Test {
        name: String,
        level: i32,
    }

    let current = Test {
        name: "hifumi".to_owned(),
        level: 1,
    };

    let downgraded = current.to_version_value("0.2")?;
    assert_eq!(
        downgraded,
        serde_json::json!({
            "$version": "0.2",
            "name": "hifumi",
            "level": 1,
            "legacy": true,
        })
    );

    let back = serde_json::from_value::<Test>(downgraded)?;
    assert_eq!(back, current);

    Ok(())
}

/// 带前向变换的 Rename 声明 `back` 后可逆（back 作用域 = 新结构体字段）。
#[test]
fn rename_with_conv_and_back_roundtrip() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        name => title: String { name.trim().to_owned() } { back: title.clone() },
    })]
    struct Test {
        title: String,
    }

    let current = Test {
        title: "hifumi".to_owned(),
    };

    let downgraded = current.to_version_value("0.1")?;
    assert_eq!(
        downgraded,
        serde_json::json!({
            "$version": "0.1",
            "name": "hifumi",
        })
    );

    let back = serde_json::from_value::<Test>(downgraded)?;
    assert_eq!(back, current);

    Ok(())
}

/// 单括号 `{ back: <expr> }` 直接写反向（无前向变换）的等价形式。
#[test]
fn rename_back_only_brace() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        name: String => title: String { back: title.clone() },
    })]
    struct Test {
        title: String,
    }

    let current = Test {
        title: "hifumi".to_owned(),
    };
    let downgraded = current.to_version_value("0.1")?;
    assert_eq!(
        downgraded,
        serde_json::json!({
            "$version": "0.1",
            "name": "hifumi",
        })
    );

    Ok(())
}

/// 多源 rename 的 back 求值为元组。
#[test]
fn multi_source_rename_back_roundtrip() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        (a: i32, b: i32) => c: String {
            (a + b).to_string()
        } {
            back: {
                let n: i32 = c.trim().parse().unwrap_or(0);
                (n / 2, n - n / 2)
            }
        },
    })]
    struct Test {
        c: String,
    }

    let current = Test { c: "7".to_owned() };

    let downgraded = current.to_version_value("0.1")?;
    assert_eq!(
        downgraded,
        serde_json::json!({
            "$version": "0.1",
            "a": 3,
            "b": 4,
        })
    );

    let back = serde_json::from_value::<Test>(downgraded)?;
    assert_eq!(back, current);

    Ok(())
}

/// 缺 back 的 Remove 一步不可逆：精确报出步与字段；该步之下所有更旧
/// 版本同样被挡（错误指向第一个不可逆步）。
#[test]
fn irreversible_reports_missing_back() -> Result<()> {
    #[version("0.4")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.3" => "0.4" {
        - legacy: bool { back: level < 2 },
    })]
    #[migration("0.2" => "0.3" {
        - gone: i32,
    })]
    #[migration("0.1" => "0.2" {
        + extra: i32,
    })]
    struct Test {
        name: String,
        level: i32,
        extra: i32,
    }

    let current = Test {
        name: "hifumi".to_owned(),
        level: 1,
        extra: 1,
    };

    // 顶步可逆：能到 0.3
    assert!(current.to_version_value("0.3").is_ok());

    // 阻断步自身（0.2）与更旧版本（0.1）都报告同一个不可逆步
    for target in ["0.1", "0.2"] {
        match current.to_version_value(target) {
            Err(DowngradeError::Irreversible { from, to, field }) => {
                assert_eq!(
                    (from.as_str(), to.as_str(), field.as_str()),
                    ("0.2", "0.3", "gone")
                );
            }
            other => panic!("expected Irreversible for {target:?}, got {other:?}"),
        }
    }

    Ok(())
}

/// 带前向变换的 Rename 缺 back 同样不可逆。
#[test]
fn rename_with_conv_without_back_irreversible() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        name => title: String { name.trim().to_owned() },
    })]
    struct Test {
        title: String,
    }

    let current = Test {
        title: "hifumi".to_owned(),
    };
    match current.to_version_value("0.1") {
        Err(DowngradeError::Irreversible { from, to, field }) => {
            assert_eq!(
                (from.as_str(), to.as_str(), field.as_str()),
                ("0.1", "0.2", "name")
            );
        }
        other => panic!("expected Irreversible, got {other:?}"),
    }

    Ok(())
}

/// 链上不存在的版本 → UnknownTarget。
#[test]
fn unknown_target() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + extra: i32,
    })]
    struct Test {
        extra: i32,
    }

    let current = Test { extra: 1 };
    match current.to_version_value("9.9") {
        Err(DowngradeError::UnknownTarget { type_name, target }) => {
            assert_eq!((type_name.as_str(), target.as_str()), ("Test", "9.9"));
        }
        other => panic!("expected UnknownTarget, got {other:?}"),
    }

    Ok(())
}

/// 错误实现 Display / Error。
#[test]
fn error_displays() {
    let error = DowngradeError::Irreversible {
        from: "0.1".to_owned(),
        to: "0.2".to_owned(),
        field: "legacy".to_owned(),
    };
    let text = error.to_string();
    assert!(text.contains(r#"step "0.1" => "0.2""#), "{text}");
    assert!(text.contains("legacy"), "{text}");
}

/// 改名目标被同一步的后续规则消费（作为多源源）时，纯改名不再免费可逆：
/// 无 back 应报 Irreversible 且指向该改名自身。修复前此 DSL 能编译但错报
/// Irreversible{field:"a, y"}（漏检被消费的 x）；编译失败（E0609）变体见
/// `consumed_rename_target_with_later_back_irreversible`。
#[test]
fn consumed_rename_target_without_back_irreversible() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        x => a: i32,
        (a: i32, y: i32) => z: i32 { a + y },
    })]
    struct Test {
        z: i32,
    }

    let current = Test { z: 5 };
    match current.to_version_value("0.1") {
        Err(DowngradeError::Irreversible { from, to, field }) => {
            assert_eq!(
                (from.as_str(), to.as_str(), field.as_str()),
                ("0.1", "0.2", "x")
            );
        }
        other => panic!("expected Irreversible, got {other:?}"),
    }

    Ok(())
}

/// E0609 回归钉：消费规则带 back、被消费的纯改名不带——修复前反向代码
/// 读取 `__new.a`（新结构体无此字段）直接编译失败，修复后正确报不可逆。
#[test]
fn consumed_rename_target_with_later_back_irreversible() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        x => a: i32,
        (a: i32, y: i32) => z: i32 { a + y } { back: (z / 2, z - z / 2) },
    })]
    struct Test {
        z: i32,
    }

    let current = Test { z: 5 };
    match current.to_version_value("0.1") {
        Err(DowngradeError::Irreversible { from, to, field }) => {
            assert_eq!(
                (from.as_str(), to.as_str(), field.as_str()),
                ("0.1", "0.2", "x")
            );
        }
        other => panic!("expected Irreversible, got {other:?}"),
    }

    Ok(())
}

/// E0063 回归钉：Copy 源被同一步先前的改名目标遮蔽（`x => a` 先消费掉
/// a，`+ a => d` 反向时无从读 a）→ 该步不可逆。修复前 checker 视 Copy
/// 恒免费，反向结构体缺 `a` 字段初始化直接编译失败。
#[test]
fn shadowed_copy_source_irreversible() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        x => a: i32,
        + a => d: i32,
    })]
    struct Test {
        a: i32,
        d: i32,
    }

    let current = Test { a: 7, d: 7 };
    match current.to_version_value("0.1") {
        Err(DowngradeError::Irreversible { from, to, field }) => {
            assert_eq!(
                (from.as_str(), to.as_str(), field.as_str()),
                ("0.1", "0.2", "a")
            );
        }
        other => panic!("expected Irreversible, got {other:?}"),
    }

    Ok(())
}

/// 同一条链声明 back 后整体可逆：被消费目标的改名从新字段反推源值，
/// 多源改名以元组 back 重建两个源字段。
#[test]
fn consumed_rename_target_with_back_roundtrip() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        x => a: i32 { back: z - z / 2 },
        (a: i32, y: i32) => z: i32 { a + y } {
            back: { let h = z / 2; (h, z - h) }
        },
    })]
    struct Test {
        z: i32,
    }

    let current = Test { z: 5 };
    let downgraded = current.to_version_value("0.1")?;
    assert_eq!(
        downgraded,
        serde_json::json!({
            "$version": "0.1",
            "x": 3,
            "a": 2,
            "y": 3,
        })
    );

    // 载荷能被正向反序列化吃回当前版本；本例的 back 恰好选成无损：
    // a + y = (z/2) + (z - z/2) = z
    let back = serde_json::from_value::<Test>(downgraded)?;
    assert_eq!(back, Test { z: 5 });

    Ok(())
}
