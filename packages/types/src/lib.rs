pub use _macros::{app_migrations, version};

pub mod app;

// Audit hifumi-R1-5: the previously-exported `MigrateInto`/`Versioned`
// traits had zero implementations and zero consumers anywhere in the
// workspace (the macro generates concrete `From` impls instead) — a
// public API promising a migration entry point that did not exist.
// Removed; reintroduce alongside a real implementation if a trait-based
// migration seam is ever needed.
