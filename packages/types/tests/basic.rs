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
    struct Test_0_2 {
        a: i32,
        b: String,
        c: i32,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[allow(non_camel_case_types)]
    struct Test_0_1 {
        a: i32,
        b: i32,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[allow(non_camel_case_types)]
    #[doc(hidden)]
    #[serde(tag = "$version")]
    enum _Test_outer {
        #[serde(rename = "0.1")]
        _0_1(Test_0_1),
        #[serde(rename = "0.2")]
        _0_2(Test_0_2),
    }

    impl From<Test_0_1> for Test_0_2 {
        fn from(t: Test_0_1) -> Self {
            Self {
                a: t.a,
                b: t.b.to_string(),
                c: 0,
            }
        }
    }

    impl From<Test_0_2> for Test {
        fn from(t: Test_0_2) -> Self {
            Self {
                a: t.a,
                b: t.b,
                c: t.c,
            }
        }
    }

    impl From<Test> for Test_0_2 {
        fn from(t: Test) -> Self {
            Self {
                a: t.a,
                b: t.b,
                c: t.c,
            }
        }
    }

    impl From<_Test_outer> for Test {
        fn from(t: _Test_outer) -> Self {
            let val: Test_0_2 = match t {
                _Test_outer::_0_1(val) => val.into(),
                _Test_outer::_0_2(val) => val,
            };
            val.into()
        }
    }

    impl From<_Test_outer> for Test_0_1 {
        fn from(t: _Test_outer) -> Self {
            match t {
                _Test_outer::_0_1(val) => val,
                _Test_outer::_0_2(_) => {
                    unimplemented!("Cannot convert new version to old version")
                }
            }
        }
    }

    impl Serialize for Test {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            _Test_outer::_0_2(self.to_owned().into()).serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for Test {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let val = _Test_outer::deserialize(deserializer)?;
            Ok(val.into())
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
