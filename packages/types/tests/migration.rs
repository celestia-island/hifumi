#[cfg(test)]
mod test {
    #[test]
    fn add_field() {
        #[version("0.2")]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            c: i32,
        }

        #[old_version(Test)]
        {
            #[version("0.1")]
            #[derive(Debug, Clone, PartialEq)]
            struct {
                a: i32,
                b: i32,
            }

            migration!("0.1" => "0.2");
        }
    }

    #[test]
    fn remove_field() {
        #[version("0.2")]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            c: i32,
        }

        #[old_version(Test)]
        {
            #[version("0.1")]
            #[derive(Debug, Clone, PartialEq)]
            struct {
                a: i32,
                b: i32,
            }

            migration!("0.1" => "0.2");
        }
    }

    #[test]
    fn rename_field() {
        #[version("0.2")]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            c: i32,
        }

        #[old_version(Test)]
        {
            #[version("0.1")]
            #[derive(Debug, Clone, PartialEq)]
            struct {
                a: i32,
                b: i32,
                d: i32,
            }

            migration!("0.1" => "0.2", {
                d <- c
            });
        }
    }

    #[test]
    fn change_field_type() {
        #[version("0.2")]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            c: String,
        }

        #[old_version(Test)]
        {
            #[version("0.1")]
            #[derive(Debug, Clone, PartialEq)]
            struct {
                a: i32,
                b: i32,
                c: i32,
            }

            migration!("0.1" => "0.2", {
                c: |val| val.to_string()
            });
        }
    }

    #[test]
    fn change_field_type_and_name() {
        #[version("0.2")]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            c: String,
        }

        #[old_version(Test)]
        {
            #[version("0.1")]
            #[derive(Debug, Clone, PartialEq)]
            struct {
                a: i32,
                b: i32,
                d: i32,
            }

            migration!("0.1" => "0.2", {
                d <- c: |val| val.to_string()
            });
        }
    }

    #[test]
    fn change_field_type_and_name_with_multiple_target() {
        #[version("0.2")]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            e: String,
            f: f32,
        }

        #[old_version(Test)]
        {
            #[version("0.1")]
            #[derive(Debug, Clone, PartialEq)]
            struct {
                a: i32,
                b: i32,
                c: i32,
                d: i32,
            }

            migration!("0.1" => "0.2", {
                e <- c: |val| val.to_string(),
                f <- d: |val| val.into()
            });
        }
    }
}
