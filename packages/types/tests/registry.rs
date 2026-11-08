//! `hifumi::registry!`：注册表宏的编译期断言与 `spec()` 汇总。
//!
//! 注册表要求类型在模块作用域可见（`__HIFUMI_META` 是类型关联常量，
//! 经 `$ty:path` 引用），故本文件的类型都在模块级声明。
//!
//! 负方向说明：`policy: backward` 注册不可逆类型是编译错误，无法用
//! 普通 `#[test]` 钉（需 trybuild 之类的编译失败基建）；这里以
//! `#[should_panic]` 在运行期钉同一条 const 断言函数的行为，编译期
//! 接线则由各通过策略的注册表本身证明（断言在 `const _` 中求值）。

use hifumi::version;

/// 全链可逆：0.1 → 0.2 仅加字段（免费可逆）。
#[version("0.2")]
#[derive(Debug, Clone, PartialEq)]
#[migration("0.1" => "0.2" {
    + b: i32,
})]
pub struct ReversibleDoc {
    pub a: i32,
    pub b: i32,
}

/// 前向兼容：无迁移 + forward 旗标（空链恒可逆，双属性齐备）。
#[version("0.2", forward)]
#[derive(Debug, Clone, PartialEq)]
pub struct LenientDoc {
    pub a: i32,
}

/// 双属性：forward 旗标 + 免费可逆链。
#[version("0.2", forward)]
#[derive(Debug, Clone, PartialEq)]
#[migration("0.1" => "0.2" {
    + b: i32,
})]
pub struct FullDoc {
    pub a: i32,
    pub b: i32,
}

/// 不可逆：rename 带转换器但无 `back`（只用于负方向运行期钉）。
#[version("0.2")]
#[derive(Debug, Clone, PartialEq)]
#[migration("0.1" => "0.2" {
    c: u8 => d: i32 { i32::from(c) },
})]
pub struct LossyDoc {
    pub d: i32,
}

mod registry_backward {
    use super::ReversibleDoc;

    hifumi::registry! {
        policy: backward {
            ReversibleDoc,
        }
    }
}

mod registry_forward {
    use super::LenientDoc;

    hifumi::registry! {
        policy: forward {
            LenientDoc,
        }
    }
}

mod registry_full {
    use super::{FullDoc, LenientDoc};

    hifumi::registry! {
        policy: full {
            FullDoc,
            LenientDoc,
        }
    }
}

#[test]
fn spec_reflects_registered_types_in_order() {
    let spec = registry_full::spec();
    assert_eq!(spec.len(), 2);

    assert_eq!(spec[0].type_name, "FullDoc");
    assert_eq!(spec[0].versions, &["0.1", "0.2"]);
    assert!(spec[0].reversible);
    assert!(spec[0].forward);

    assert_eq!(spec[1].type_name, "LenientDoc");
    assert_eq!(spec[1].versions, &["0.2"]);
    assert!(spec[1].reversible);
    assert!(spec[1].forward);

    assert_eq!(registry_backward::spec().len(), 1);
    assert_eq!(registry_forward::spec().len(), 1);
}

/// 同一函数体的 const 上下文求值通过：证明断言确实是 const fn，
/// 而不是只在运行期才检查。
const _: () = hifumi::registry_assert_reversible(&ReversibleDoc::__HIFUMI_META);
const _: () = hifumi::registry_assert_forward(&LenientDoc::__HIFUMI_META);

#[test]
#[should_panic(expected = "policy: backward")]
fn reversible_assert_fires_on_irreversible_meta() {
    hifumi::registry_assert_reversible(&LossyDoc::__HIFUMI_META);
}

#[test]
#[should_panic(expected = "policy: forward")]
fn forward_assert_fires_on_unflagged_meta() {
    hifumi::registry_assert_forward(&ReversibleDoc::__HIFUMI_META);
}

/// 空注册表：合法，spec() 为空切片。
mod registry_empty {
    hifumi::registry! {
        policy: backward {}
    }
}

#[test]
fn empty_registry_yields_empty_spec() {
    assert!(registry_empty::spec().is_empty());
}
