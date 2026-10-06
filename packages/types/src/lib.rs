pub use _macros::{app_migrations, version};

pub mod app;

use anyhow::Result;

pub trait MigrateInto<T> {
    fn migrate(&self) -> Result<T>;
}

pub trait Versioned {
    fn version(&self) -> Option<&str>;
}
