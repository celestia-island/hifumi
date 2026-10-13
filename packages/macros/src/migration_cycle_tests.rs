//! Audit hifumi-R1-1: the migration-chain walk must refuse cyclic DSL
//! input with a compile error instead of hanging the compiler.

use super::*;

// The walk lives in template::impl_migration (Result-returning); the
// lib.rs version() walk returns TokenStream directly. Both share the
// visited-set guard. These tests exercise the Result path via the
// template helper.
