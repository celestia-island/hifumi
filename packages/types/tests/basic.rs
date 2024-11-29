#[cfg(test)]
mod test {
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
    fn decl_old_version() {
        #[version("0.2")]
        #[derive(Debug, Clone, PartialEq)]
        struct Test {
            a: i32,
            b: i32,
            c: i32,
        }

        #[old_version]
        {
            #[version("0.1")]
            #[derive(Debug, Clone, PartialEq)]
            struct {
                a: i32,
                b: i32,
                c: i32,
            }

            migration!("0.1" => "0.2");
        }
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

        #[old_version]
        {
            #[version("0.2")]
            #[derive(Debug, Clone, PartialEq)]
            struct {
                a: i32,
                b: i32,
                c: i32,
            }

            migration!("0.2" => "0.3");

            #[version("0.1")]
            #[derive(Debug, Clone, PartialEq)]
            struct {
                a: i32,
                b: i32,
            }

            migration!("0.1" => "0.2");
        }
    }
}
