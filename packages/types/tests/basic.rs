use anyhow::Result;
use hifumi::version;
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
    #[migration("0.1" => "0.2")]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }
}

#[test]
fn decl_old_version_expand() -> Result<()> {
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: String,
        c: i32,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[allow(non_camel_case_types)]
    #[doc(hidden)]
    #[serde(untagged)]
    enum _Test_0_1_b {
        _0_1(i32),
        _0_2(String),
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[allow(non_camel_case_types)]
    #[doc(hidden)]
    struct _Test_outer {
        #[doc(hidden)]
        #[serde(rename = "$version")]
        version: String,

        a: Option<i32>,
        b: Option<_Test_0_1_b>,
        c: Option<i32>,
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
                b: t.b.to_string(),
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
                a: Some(self.a.clone()),
                b: Some(_Test_0_1_b::_0_2(self.b.clone())),
                c: Some(self.c.clone()),
            }
            .serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for Test {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let value = _Test_outer::deserialize(deserializer)?;

            match value.version.as_str() {
                "0.1" => {
                    let value = Test_0_1 {
                        a: value.a.unwrap(),
                        b: match value.b.unwrap() {
                            _Test_0_1_b::_0_1(b) => b,
                            _ => return Err(serde::de::Error::custom("Invalid version")),
                        },
                    };
                    Ok(Test::from(value))
                }
                "0.2" => Ok(Test {
                    a: value.a.unwrap(),
                    b: match value.b.unwrap() {
                        _Test_0_1_b::_0_2(b) => b,
                        _ => return Err(serde::de::Error::custom("Invalid version")),
                    },
                    c: value.c.unwrap(),
                }),
                _ => Err(serde::de::Error::custom("Invalid version")),
            }
        }
    }

    assert_eq!(
        "{\"$version\":\"0.2\",\"a\":1,\"b\":\"2\",\"c\":3}",
        serde_json::to_string(&Test {
            a: 1,
            b: "2".to_string(),
            c: 3
        })?
    );
    assert_eq!(
        Test {
            a: 1,
            b: "2".to_string(),
            c: 3
        },
        serde_json::from_str::<Test>("{\"$version\":\"0.2\",\"a\":1,\"b\":\"2\",\"c\":3}")?
    );

    assert_eq!(
        serde_json::from_str::<Test>("{\"$version\":\"0.1\",\"a\":1,\"b\":2}")?,
        Test {
            a: 1,
            b: "2".to_string(),
            c: 0
        }
    );

    Ok(())
}

#[test]
fn decl_multiple_old_versions() {
    #[version("0.3")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.2" => "0.3" {
            + c: i32
        })]
    #[migration("0.1" => "0.2")]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }
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
