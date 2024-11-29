#[cfg(test)]
mod test {
    #[test]
    fn add_field() {
        #[version(0.2)]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            c: i32,
        }

        migration!(Test 0.1 => 0.2 {
            + c: i32
        });
    }

    #[test]
    fn remove_field() {
        #[version(0.2)]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
        }

        migration!(Test 0.1 => 0.2 {
            - c: i32
        });
    }

    #[test]
    fn rename_field() {
        #[version(0.2)]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            c: i32,
        }

        migration!(Test 0.1 => 0.2 {
            c => d: i32
        });
    }

    #[test]
    fn copy_field() {
        #[version(0.2)]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            c: i32,
        }

        migration!(Test 0.1 => 0.2 {
            + c => d: i32
        });
    }

    #[test]
    fn change_field_type() {
        #[version(0.2)]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            c: String,
        }

        migration!(Test 0.1 => 0.2 {
            c: i32 => String |val| val.to_string()
        });
    }

    #[test]
    fn change_field_type_and_name() {
        #[version(0.2)]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            c: String,
        }

        migration!(Test 0.1 => 0.2 {
            c: i32 => d: String |val| val.to_string()
        });
    }

    #[test]
    fn change_field_type_and_name_with_multiple_target() {
        #[version(0.2)]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            e: String,
            f: f32,
        }

        migration!(Test 0.1 => 0.2 {
            c: i32 => e: String |val| val.to_string(),
            c: i32 => f: f32 |val| val.into(),
        });
    }

    #[test]
    fn change_field_type_and_name_with_multiple_source() {
        #[version(0.2)]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            e: String,
            f: f32,
        }

        migration!(Test 0.1 => 0.2 {
            (c: i32, d: i32) => e: String |(c, d)| (c + d).to_string(),
        });
    }

    #[test]
    fn copy_field_with_multiple_target() {
        #[version(0.2)]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            e: String,
            f: f32,
        }

        migration!(Test 0.1 => 0.2 {
            + c => e: String,
            + d => f: f32,
        });
    }

    #[test]
    fn copy_field_with_multiple_source() {
        #[version(0.2)]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            e: String,
            f: f32,
        }

        migration!(Test 0.1 => 0.2 {
            + (c, d) => e: String,
        });
    }

    #[test]
    fn remove_field_with_multiple_source() {
        #[version(0.2)]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            e: String,
            f: f32,
        }

        migration!(Test 0.1 => 0.2 {
            (c, d) => e: String,
        });
    }
}
