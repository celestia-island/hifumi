//! `export-ts` 的金样夹具：两个 `#[version]` 类型。
//!
//! - `Doc`：三步链 + `baseline = "0.2"`（`0.1 => 0.2` 步在导出时灭绝截断），
//!   覆盖全部可导出 op 形态，含 back 括号（前向无关）与保名换型。
//! - `Solo`：无迁移步的单版本链（`baseline == final` 的极端形态由
//!   `Doc` 截断后的边界覆盖，这里覆盖「零步」本身）。

#[version("0.3", baseline = "0.2")]
#[migration("0.1" => "0.2" {
    - extra: String { back: String::new() },
    + doc_kind: String,
    + tags: Vec<String>,
    + stamp: u32,
    + flag: bool,
    + note: Option<String>,
    + scores: std::collections::BTreeMap<String, i32>,
    + n: f64,
    body: String => content: String,
    title => heading: String,
    old: String => copy_of_old: String,
    + alias: String => alias_copy: String,
})]
#[migration("0.2" => "0.3" {
    + pinned: bool,
    - dead: String { back: String::new() },
    alias: String => label: String { back: __new.label.clone() },
    + label: String => echo: String,
})]
pub struct Doc {
    pub content: String,
    pub heading: String,
    pub pinned: bool,
    pub label: String,
    pub echo: String,
}

#[version("0.9")]
pub struct Solo {
    pub id: u32,
}
