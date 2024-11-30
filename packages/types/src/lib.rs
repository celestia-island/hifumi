pub use _macros::{migration, version};

use anyhow::Result;
use chrono::{DateTime, Utc};
use semver::Version;

pub trait MigrateInto<T> {
    fn migrate(&self) -> Result<T>;
}

pub trait Versioned {
    fn version(&self) -> Option<(Version, DateTime<Utc>)>;
}
