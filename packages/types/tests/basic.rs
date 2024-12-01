use anyhow::Result;
use hifumi::{migration, version};
use serde::{Deserialize, Serialize};

#[test]
fn decl_single_version() {
    #[version("0.1")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }
}

#[test]
fn decl_single_version_expand() -> Result<()> {
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[allow(non_camel_case_types)]
    #[doc(hidden)]
    struct _Test_outer {
        #[doc(hidden)]
        #[serde(rename = "$version")]
        version: String,

        a: i32,
        b: i32,
        c: i32,
    }

    impl Serialize for Test {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            _Test_outer {
                version: "0.1".to_string(),
                a: self.a,
                b: self.b,
                c: self.c,
            }
            .serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for Test {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let _Test_outer { a, b, c, .. } = _Test_outer::deserialize(deserializer)?;
            Ok(Test { a, b, c })
        }
    }

    assert_eq!(
        "{\"$version\":\"0.1\",\"a\":1,\"b\":2,\"c\":3}",
        serde_json::to_string(&Test { a: 1, b: 2, c: 3 })?
    );
    assert_eq!(
        Test { a: 1, b: 2, c: 3 },
        serde_json::from_str::<Test>("{\"$version\":\"0.1\",\"a\":1,\"b\":2,\"c\":3}")?
    );

    Ok(())
}

#[test]
fn decl_single_version_with_suffix() {
    #[version("1.0.0-rc12")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }
}

#[test]
fn decl_old_version() {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    migration!(Test "0.1" => "0.2");
}

#[test]
fn decl_old_version_expand() -> Result<()> {
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[allow(non_camel_case_types)]
    #[doc(hidden)]
    struct _Test_outer {
        #[doc(hidden)]
        #[serde(rename = "$version")]
        version: String,

        a: i32,
        b: i32,
        c: i32,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[allow(non_camel_case_types)]
    struct Test_0_1 {
        a: i32,
        b: i32,
    }

    impl From<Test_0_1> for Test {
        fn from(t: Test_0_1) -> Self {
            Self {
                a: t.a,
                b: t.b,
                c: 0,
            }
        }
    }

    impl Serialize for Test {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            _Test_outer {
                version: "0.2".to_string(),
                a: self.a,
                b: self.b,
                c: self.c,
            }
            .serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for Test {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let _Test_outer { a, b, c, .. } = _Test_outer::deserialize(deserializer)?;
            Ok(Test { a, b, c })
        }
    }

    assert_eq!(
        "{\"$version\":\"0.2\",\"a\":1,\"b\":2,\"c\":0}",
        serde_json::to_string(&Test { a: 1, b: 2, c: 3 })?
    );
    assert_eq!(
        Test { a: 1, b: 2, c: 0 },
        serde_json::from_str::<Test>("{\"$version\":\"0.2\",\"a\":1,\"b\":2,\"c\":3}")?
    );

    assert_eq!(
        "{\"$version\":\"0.1\",\"a\":1,\"b\":2}",
        serde_json::to_string(&Test { a: 1, b: 2, c: 0 })?
    );

    Ok(())
}

#[test]
fn decl_multiple_old_versions() {
    #[version("0.3")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    migration!(Test "0.2" => "0.3" {
        + c: i32
    });
    migration!(Test "0.1" => "0.2");
}

#[test]
fn decl_version_with_timestamp() {
    #[version("2025-1-1")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    #[version("2025-01-01")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test2 {
        a: i32,
        b: i32,
        c: i32,
    }

    #[version("2025-1-1 9:34")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test3 {
        a: i32,
        b: i32,
        c: i32,
    }

    #[version("2025-1-01 09:34:12")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test4 {
        a: i32,
        b: i32,
        c: i32,
    }

    #[version("12345678")]
    #[derive(Debug, Clone, PartialEq)]
    struct Test5 {
        a: i32,
        b: i32,
        c: i32,
    }
}
