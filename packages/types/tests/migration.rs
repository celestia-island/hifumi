use hifumi::{migration, version};

#[test]
fn add_field() {
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
}

#[test]
fn remove_field() {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        - c: i32
    })]
    struct Test {
        a: i32,
        b: i32,
    }
}

#[test]
fn rename_field() {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        c => d: i32
    })]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }
}

#[test]
fn copy_field() {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + c => d: i32
    })]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }
}

#[test]
fn add_field_with_default_value() {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + c: i32 { 42 }
    })]
    struct Test {
        a: i32,
        b: i32,
    }
}

#[test]
fn rename_field_with_default_value() {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        c => d: i32 { 42 }
    })]
    struct Test {
        a: i32,
        b: i32,
        c: i32,
    }
}

#[test]
fn change_field_type() {
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
}

#[test]
fn change_field_type_and_name() {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        c: i32 => d: String { c.to_string() }
    })]
    struct Test {
        a: i32,
        b: i32,
        c: String,
    }
}

#[test]
fn change_field_type_and_name_with_multiple_target() {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        c: i32 => e: String { c.to_string() },
        c: i32 => f: f32 { c.into() },
    })]
    struct Test {
        a: i32,
        b: i32,
        e: String,
        f: f32,
    }
}

#[test]
fn change_field_type_and_name_with_multiple_source() {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        (c: i32, d: i32) => e: String { (c + d).to_string() },
    })]
    struct Test {
        a: i32,
        b: i32,
        e: String,
        f: f32,
    }
}

#[test]
fn copy_field_with_multiple_target() {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + c => e: String,
        + d => f: f32,
    })]
    struct Test {
        a: i32,
        b: i32,
        e: String,
        f: f32,
    }
}

#[test]
fn copy_field_with_multiple_source() {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        + (c: i32, d: i32) => e: String { (c + d).to_string() },
    })]
    struct Test {
        a: i32,
        b: i32,
        e: String,
        f: f32,
    }
}

#[test]
fn remove_field_with_multiple_source() {
    #[version("0.2")]
    #[derive(Debug, Clone, PartialEq)]
    #[migration("0.1" => "0.2" {
        (c: i32, d: i32) => e: String { (c + d).to_string() },
    })]
    struct Test {
        a: i32,
        b: i32,
        e: String,
        f: f32,
    }
}
