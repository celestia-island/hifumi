use anyhow::Result;
use hifumi::version;
use specta::Type;

#[test]
fn specta_basic() -> Result<()> {
    // 测试 specta 的 Type derive 能够与 hifumi 一起使用
    #[version("0.1")]
    #[derive(Debug, Clone, PartialEq, Type)]
    struct Test {
        a: i32,
        b: String,
        c: bool,
    }

    // 验证序列化仍然正常工作
    let test = Test {
        a: 1,
        b: "hello".to_string(),
        c: true,
    };
    let json = serde_json::to_string(&test)?;
    assert!(json.contains(r#""$version":"0.1""#));
    assert!(json.contains(r#""a":1"#));
    assert!(json.contains(r#""b":"hello""#));
    assert!(json.contains(r#""c":true"#));

    // 验证 specta 类型可以导出
    let ts = specta::ts::export::<Test>(&Default::default())?;
    assert!(ts.contains("Test"));

    Ok(())
}

#[test]
fn specta_with_migration() -> Result<()> {
    // 测试带有迁移规则的结构体也能与 specta 一起使用
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq, Type)]
    #[migration("0.1" => "0.2" {
        + c: bool,
    })]
    struct Test {
        a: i32,
        b: String,
        c: bool,
    }

    // 从 0.1 版本迁移
    let test: Test = serde_json::from_str(r#"{"$version":"0.1","a":1,"b":"hello"}"#)?;
    assert_eq!(test.a, 1);
    assert_eq!(test.b, "hello");
    assert_eq!(test.c, false); // 默认值

    // 从 0.2 版本直接解析
    let test: Test = serde_json::from_str(r#"{"$version":"0.2","a":1,"b":"hello","c":true}"#)?;
    assert_eq!(test.c, true);

    Ok(())
}

#[test]
fn specta_typescript_export() -> Result<()> {
    // 测试 specta 能够导出 TypeScript 类型定义
    #[version("0.1")]
    #[derive(Debug, Clone, PartialEq, Type)]
    struct User {
        id: i32,  // 使用 i32 而不是 i64，因为 specta 默认不支持 BigInt
        name: String,
        email: Option<String>,
        tags: Vec<String>,
    }

    // 使用 specta 生成 TypeScript 类型
    let ts_type = specta::ts::export::<User>(&Default::default())?;

    // 验证生成的 TypeScript 类型包含所有字段
    assert!(ts_type.contains("id: number"));
    assert!(ts_type.contains("name: string"));
    assert!(ts_type.contains("email: string | null"));
    assert!(ts_type.contains("tags: string[]"));

    Ok(())
}
