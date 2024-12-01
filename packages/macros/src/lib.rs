use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemStruct};

mod template;
mod tools;

use template::generate_current_version_struct;
use tools::{DeriveVersion, Migration};

#[proc_macro_attribute]
pub fn version(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attr: DeriveVersion = parse_macro_input!(attr);

    // Get the struct's name
    let input = parse_macro_input!(input as ItemStruct);
    let name = &input.ident;

    generate_current_version_struct(name.clone(), attr)
        .expect("Failed to generate current version struct")
        .into()
}

#[proc_macro]
pub fn migration(input: TokenStream) -> TokenStream {
    let attr: Migration = parse_macro_input!(input);

    /*
        生成的大概长这样：

        #[derive(Debug, Clone, PartialEq, ::hifumi::Serialize, ::hifumi::Deserialize)]
        struct _Test_outer {
            #[doc(hidden)]
            #[serde(rename = "$version")]
            version: String,

            #[serde(flatten)]
            inner: Test,
        }

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]  // 这里还用普通的 Serialize 和 Deserialize
        struct Test {
            a: i32,
            b: i32,
            c: i32,
        }

        // 省略 Test_0_1 和 Test_0_2 的定义
        // 注意这里使用这种名字单纯只是为了方便阅读，实际生成时，为了能接受各种奇怪写法的 struct 名字，
        // 名字本身会直接丢 sqids 加盐转换，盐是这个 struct 的本体名称，这样就不会有重名问题了，如 Test_B4aajs
        // 后续可能会开放给 version 宏额外的配置手段，例如加长 sqids 生成的编号长度等

        impl From<Test_0_1> for Test_0_2 {
            fn from(t: Test_0_1) -> Self {
                Self {
                    a: t.a,
                    b: t.b,
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

        impl Version for Test {
            fn version(&self) -> String {
                "0.3".to_string()
            }
        }

        hifumi::Serialize 就先省略了，其实就是在 Serialize 过程中自动加上 $version 字段，把 version() 的返回值写入进去
        可以通过特殊的 Trait 实现访问旧版本 struct 的 Serialize，以获取一个旧版本的 JSON 结果

        然后 hifumi::Deserialize 这么干：

        impl ::hifumi::Deserialize for Test {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::Deserializer,
            {
                // 提前分析是否有个叫 $version 的字段，如果有，就读取出来，并且最终不要写入到 Test 结构体中
                // 如果没有，默认按最新版本的来直接读取

                // 读取 $version 字段
                let version: String = match deserializer.deserialize_map(Visitor)? {
                    Some(version) => version,
                    None => "0.3".to_string(),
                };

                // 读取其他字段，按默认 Deserialize 的逻辑运行
                // 这里就先省略了，调用的是这个结构的默认 serde::Deserialize 实现

                // 先根据声明的版本反序列化为对应版本的结构体
                // 然后如果不是最新版本，搜索和执行上游若干层版本的迁移，直到转移到最新版本
                match version.as_str() {
                    "0.1" => {
                        let ret: Test_0_1 = ::serde::Deserialize::deserialize(deserializer)?;
                        let ret: Test_0_2 = ret.into();
                        let ret: Test = ret.into();
                        Ok(ret)
                    }
                    "0.2" => {
                        let ret: Test_0_2 = ::serde::Deserialize::deserialize(deserializer)?;
                        let ret: Test = ret.into();
                        Ok(ret)
                    }
                    "0.3" => {
                        let ret: Test = ::serde::Deserialize::deserialize(deserializer)?;
                        Ok(ret)
                    }
                    _ => Err(::serde::de::Error::custom("Unknown version")),
                }
            }
        }
    */

    quote! {}.into()
}
