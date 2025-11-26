use anyhow::Result;
use hifumi::version;

#[test]
fn decl_single_version() -> Result<()> {
    #[version("0.1")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    assert_eq!(
        r#"{"$version":"0.1","a":1,"b":2,"c":3}"#,
        serde_json::to_string(&Test { a: 1, b: 2, c: 3 })?
    );
    assert_eq!(
        serde_json::to_string(&Test { a: 1, b: 2, c: 3 })?,
        r#"{"$version":"0.1","a":1,"b":2,"c":3}"#,
    );

    Ok(())
}

#[test]
fn decl_single_version_with_suffix() -> Result<()> {
    #[version("1.0.0-rc12")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    assert_eq!(
        r#"{"$version":"1.0.0-rc12","a":1,"b":2,"c":3}"#,
        serde_json::to_string(&Test { a: 1, b: 2, c: 3 })?
    );
    assert_eq!(
        serde_json::to_string(&Test { a: 1, b: 2, c: 3 })?,
        r#"{"$version":"1.0.0-rc12","a":1,"b":2,"c":3}"#,
    );

    Ok(())
}

#[test]
fn decl_old_version() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2")]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2,"c":3}"#)?,
        Test { a: 1, b: 2, c: 3 }
    );

    Ok(())
}

#[test]
fn decl_multiple_old_versions() -> Result<()> {
    #[version("0.5")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.4" => "0.5" {
        + d: i32,
    })]
    #[migration("0.3" => "0.4" {
        c: i32 => String { c.to_string() },
    })]
    #[migration("0.2" => "0.3" {
        + c: i32
    })]
    #[migration("0.1" => "0.2")]
    struct Test {
        a: i32,
        b: i32,
        c: String,
        d: i32,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.5","a":1,"b":2,"c":"3","d":4}"#)?,
        Test {
            a: 1,
            b: 2,
            c: "3".to_string(),
            d: 4
        }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.4","a":1,"b":2,"c":"3"}"#)?,
        Test {
            a: 1,
            b: 2,
            c: "3".to_string(),
            d: 0
        }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.3","a":1,"b":2,"c":3}"#)?,
        Test {
            a: 1,
            b: 2,
            c: "3".to_string(),
            d: 0
        }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":1,"b":2}"#)?,
        Test {
            a: 1,
            b: 2,
            c: "0".to_string(),
            d: 0
        }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2}"#)?,
        Test {
            a: 1,
            b: 2,
            c: "0".to_string(),
            d: 0
        }
    );

    Ok(())
}

#[test]
fn decl_version_with_timestamp() -> Result<()> {
    #[version("2025-1-1")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    assert_eq!(
        r#"{"$version":"2025-1-1","a":1,"b":2,"c":3}"#,
        serde_json::to_string(&Test { a: 1, b: 2, c: 3 })?
    );
    assert_eq!(
        serde_json::to_string(&Test { a: 1, b: 2, c: 3 })?,
        r#"{"$version":"2025-1-1","a":1,"b":2,"c":3}"#,
    );

    #[version("2025-01-01")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test2 {
        a: i32,
        b: i32,
        c: i32,
    }

    assert_eq!(
        r#"{"$version":"2025-01-01","a":1,"b":2,"c":3}"#,
        serde_json::to_string(&Test2 { a: 1, b: 2, c: 3 })?
    );
    assert_eq!(
        serde_json::to_string(&Test2 { a: 1, b: 2, c: 3 })?,
        r#"{"$version":"2025-01-01","a":1,"b":2,"c":3}"#,
    );

    #[version("2025-1-1 9:34")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test3 {
        a: i32,
        b: i32,
        c: i32,
    }

    assert_eq!(
        r#"{"$version":"2025-1-1 9:34","a":1,"b":2,"c":3}"#,
        serde_json::to_string(&Test3 { a: 1, b: 2, c: 3 })?
    );
    assert_eq!(
        serde_json::to_string(&Test3 { a: 1, b: 2, c: 3 })?,
        r#"{"$version":"2025-1-1 9:34","a":1,"b":2,"c":3}"#,
    );

    #[version("2025-1-01 09:34:12")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test4 {
        a: i32,
        b: i32,
        c: i32,
    }

    assert_eq!(
        r#"{"$version":"2025-1-01 09:34:12","a":1,"b":2,"c":3}"#,
        serde_json::to_string(&Test4 { a: 1, b: 2, c: 3 })?
    );
    assert_eq!(
        serde_json::to_string(&Test4 { a: 1, b: 2, c: 3 })?,
        r#"{"$version":"2025-1-01 09:34:12","a":1,"b":2,"c":3}"#,
    );

    #[version("12345678")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test5 {
        a: i32,
        b: i32,
        c: i32,
    }

    assert_eq!(
        r#"{"$version":"12345678","a":1,"b":2,"c":3}"#,
        serde_json::to_string(&Test5 { a: 1, b: 2, c: 3 })?
    );
    assert_eq!(
        serde_json::to_string(&Test5 { a: 1, b: 2, c: 3 })?,
        r#"{"$version":"12345678","a":1,"b":2,"c":3}"#,
    );

    Ok(())
}

#[test]
fn decl_version_auto() -> Result<()> {
    // 使用无参数的 #[version] 宏，将自动使用 CARGO_PKG_VERSION
    #[version]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    // 获取当前 crate 版本
    let expected_version = env!("CARGO_PKG_VERSION");
    let expected = format!(r#"{{"$version":"{}","a":1,"b":2,"c":3}}"#, expected_version);

    assert_eq!(expected, serde_json::to_string(&Test { a: 1, b: 2, c: 3 })?);

    Ok(())
}
