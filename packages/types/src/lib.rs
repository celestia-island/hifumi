pub use _macros::{app_migrations, version};

pub mod app;
pub mod downgrade;
pub mod forward;
pub mod registry;

pub use downgrade::{__downgrade_json, DowngradeError};
pub use forward::{Forward, TryFromValue};
pub use registry::{registry_assert_forward, registry_assert_reversible, TypeMeta};

// 宏生成的 `to_version_value` 签名通过 `::hifumi::serde_json::` 引用 JSON
// 类型；re-export 免去下游 crate 为此直接依赖 serde_json。
pub use serde_json;

// Audit hifumi-R1-5: the previously-exported `MigrateInto`/`Versioned`
// traits had zero implementations and zero consumers anywhere in the
// workspace (the macro generates concrete `From` impls instead) — a
// public API promising a migration entry point that did not exist.
// Removed; reintroduce alongside a real implementation if a trait-based
// migration seam is ever needed.
