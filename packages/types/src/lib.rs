pub use _macros::version;

use anyhow::Result;

pub trait MigrateInto<T> {
    fn migrate(&self) -> Result<T>;
}

pub trait Versioned {
    fn version(&self) -> Option<&str>;
}
