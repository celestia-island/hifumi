use hifumi::{migration, version};

#[test]
fn decl_single_version() {
    #[version(0.1)]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }
}

#[test]
fn decl_single_version_with_suffix() {
    #[version(1.0.0-rc12)]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }
}

#[test]
fn decl_old_version() {
    #[version(0.2)]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    migration!(0.1 => 0.2);
}

#[test]
fn decl_multiple_old_versions() {
    #[version(0.3)]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    migration!(0.2 => 0.3 {
        + c: i32
    });
    migration!(0.1 => 0.2);
}

#[test]
fn decl_version_with_timestamp() {
    #[version(2025-1-1)]
    #[derive(Debug, Clone, PartialEq)]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }

    #[version(2025-01-01)]
    #[derive(Debug, Clone, PartialEq)]
    struct Test2 {
        a: i32,
        b: i32,
        c: i32,
    }

    #[version(2025-1-1 9:34)]
    #[derive(Debug, Clone, PartialEq)]
    struct Test3 {
        a: i32,
        b: i32,
        c: i32,
    }

    #[version(2025-1-01 09:34:12)]
    #[derive(Debug, Clone, PartialEq)]
    struct Test4 {
        a: i32,
        b: i32,
        c: i32,
    }

    #[version(unix(12345678))]
    #[derive(Debug, Clone, PartialEq)]
    struct Test5 {
        a: i32,
        b: i32,
        c: i32,
    }
}
