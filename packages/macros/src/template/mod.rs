pub(crate) mod current_version_struct;
pub(crate) mod impl_migration;
pub(crate) mod old_version_structs;
pub(crate) mod schema_builder;

pub(crate) use current_version_struct::generate_current_version_struct;
pub(crate) use impl_migration::generate_impl_migration;
pub(crate) use old_version_structs::generate_old_version_structs;
