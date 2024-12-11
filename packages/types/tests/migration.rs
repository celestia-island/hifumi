use anyhow::Result;
use hifumi::version;

#[test]
fn add_field() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + c: i32
    })]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2}"#)?,
        Test { a: 1, b: 2, c: 0 }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":1,"b":2,"c":3}"#)?,
        Test { a: 1, b: 2, c: 3 }
    );

    Ok(())
}

#[test]
fn remove_field() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        - c: i32
    })]
    struct Test {
        a: i32,
        b: i32,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2,"c":3}"#)?,
        Test { a: 1, b: 2 }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":1,"b":2}"#)?,
        Test { a: 1, b: 2 }
    );

    Ok(())
}

#[test]
fn rename_field() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        c => d: i32
    })]
    struct Test {
        a: i32,
        b: i32,
        d: i32,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2,"c":3}"#)?,
        Test { a: 1, b: 2, d: 3 }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":1,"b":2,"d":3}"#)?,
        Test { a: 1, b: 2, d: 3 }
    );

    Ok(())
}

#[test]
fn copy_field() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + c => d: i32
    })]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
        d: i32,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2,"c":3}"#)?,
        Test {
            a: 1,
            b: 2,
            c: 3,
            d: 3
        }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":1,"b":2,"c":3,"d":4}"#)?,
        Test {
            a: 1,
            b: 2,
            c: 3,
            d: 4
        }
    );

    Ok(())
}

#[test]
fn add_field_with_default_value() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + c: i32 { 42 }
    })]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2}"#)?,
        Test { a: 1, b: 2, c: 42 }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":1,"b":2,"c":3}"#)?,
        Test { a: 1, b: 2, c: 3 }
    );

    Ok(())
}

#[test]
fn rename_field_with_default_value() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        c => d: i32 { 42 }
    })]
    struct Test {
        a: i32,
        b: i32,
        d: i32,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2,"c":3}"#)?,
        Test { a: 1, b: 2, d: 42 }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":1,"b":2,"d":3}"#)?,
        Test { a: 1, b: 2, d: 3 }
    );

    Ok(())
}

#[test]
fn change_field_type() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        c: i32 => String { c.to_string() }
    })]
    struct Test {
        a: i32,
        b: i32,
        c: String,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2,"c":3}"#)?,
        Test {
            a: 1,
            b: 2,
            c: "3".to_string()
        }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":1,"b":2,"c":"3"}"#)?,
        Test {
            a: 1,
            b: 2,
            c: "3".to_string()
        }
    );

    Ok(())
}

#[test]
fn change_field_type_and_name() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        c: i32 => d: String { c.to_string() }
    })]
    struct Test {
        a: i32,
        b: i32,
        d: String,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2,"c":3}"#)?,
        Test {
            a: 1,
            b: 2,
            d: "3".to_string()
        }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":1,"b":2,"d":"3"}"#)?,
        Test {
            a: 1,
            b: 2,
            d: "3".to_string()
        }
    );

    Ok(())
}

#[test]
fn change_field_type_and_name_with_multiple_target() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        c: i32 => e: String { c.to_string() },
        c: i32 => f: f32 { c as f32 },
    })]
    struct Test {
        a: i32,
        b: i32,
        e: String,
        f: f32,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2,"c":3}"#)?,
        Test {
            a: 1,
            b: 2,
            e: "3".to_string(),
            f: 3.
        }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":1,"b":2,"e":"3","f":3.0}"#)?,
        Test {
            a: 1,
            b: 2,
            e: "3".to_string(),
            f: 3.
        }
    );

    Ok(())
}

#[test]
fn change_field_type_and_name_with_multiple_source() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        (c: i32, d: i32) => e: String { (c + d).to_string() },
    })]
    struct Test {
        a: i32,
        b: i32,
        e: String,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2,"c":3,"d":4}"#)?,
        Test {
            a: 1,
            b: 2,
            e: "7".to_string(),
        }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":1,"b":2,"e":"7"}"#)?,
        Test {
            a: 1,
            b: 2,
            e: "7".to_string(),
        }
    );

    Ok(())
}

#[test]
fn copy_field_with_multiple_target() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + a=> c: String,
        + b => d: f32,
    })]
    struct Test {
        a: String,
        b: f32,
        c: String,
        d: f32,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":"1","b":2.0}"#)?,
        Test {
            a: "1".to_string(),
            b: 2.,
            c: "1".to_string(),
            d: 2.,
        }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":"1","b":2.0,"c":"1","d":2.0}"#)?,
        Test {
            a: "1".to_string(),
            b: 2.,
            c: "1".to_string(),
            d: 2.,
        }
    );

    Ok(())
}

#[test]
fn copy_field_with_multiple_source() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + (a: i32, b: i32) => c: String { (a + b).to_string() },
    })]
    struct Test {
        a: i32,
        b: i32,
        c: String,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2}"#)?,
        Test {
            a: 1,
            b: 2,
            c: "3".to_string(),
        }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","a":1,"b":2,"c":"3"}"#)?,
        Test {
            a: 1,
            b: 2,
            c: "3".to_string(),
        }
    );

    Ok(())
}

#[test]
fn remove_field_with_multiple_source() -> Result<()> {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        (a: i32, b: i32) => c: String { (a + b).to_string() },
    })]
    struct Test {
        c: String,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.1","a":1,"b":2}"#)?,
        Test { c: "3".to_string() }
    );
    assert_eq!(
        serde_json::from_str::<Test>(r#"{"$version":"0.2","c":"3"}"#)?,
        Test { c: "3".to_string() }
    );

    Ok(())
}
