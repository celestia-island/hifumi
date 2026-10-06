<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/hifumi/master/docs/logo.webp" alt="Hifumi" width="240" /></p>

![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/celestia-island/hifumi/test.yml)

## Introduction

A serialization library for migrating data between different versions.

The name `hifumi` comes from the character [Hifumi](https://bluearchive.wiki/wiki/hifumi) in the game [Blue Archive](https://bluearchive.jp/).

> Still in development, the API may change in the future.

## Quick Start

```rust
use hifumi::version;

#[version("0.2")]
#[derive(Debug, Clone, PartialEq)]
#[migration("0.1" => "0.2" {
    + (c: i32, d: i32) => e: String { (c + d).to_string() },
    - f: f32,
})]
struct Test {
    a: i32,
    b: i32,
    c: i32,
    d: i32,
    e: String,
}
```

## Features

### Automatic Version Detection

You can use `#[version]` without arguments to automatically use `CARGO_PKG_VERSION`:

```rust
use hifumi::version;

#[version]  // Automatically uses CARGO_PKG_VERSION
#[derive(Debug, Clone, PartialEq)]
struct Config {
    // ...
}
```

### TypeScript Type Export (specta)

Hifumi supports [specta](https://github.com/specta-rs/specta) for TypeScript type generation:

```rust
use hifumi::version;
use specta::Type;

#[version("0.1")]
#[derive(Debug, Clone, PartialEq, Type)]  // Add Type derive
struct User {
    id: i32,
    name: String,
}

// Export to TypeScript
let ts = specta::ts::export::<User>(&Default::default())?;
```

### App-Version One-Time Actions

Besides migrating data structures, hifumi scaffolds actions that run **once** when the application itself upgrades across a version — repairs, preference rewrites, or anything that must happen exactly one time per profile:

```rust
use hifumi::app_migrations;

#[app_migrations]  // Current app version; defaults to CARGO_PKG_VERSION
mod app_migrations_registry {
    use anyhow::Result;
    use hifumi::app::AppMigrationContext;

    /// Runs once when an upgrade crosses into "0.5.2", from any older version.
    #[once("0.5.2")]
    fn rewrite_config(_ctx: &AppMigrationContext) -> Result<()> {
        // ...
        Ok(())
    }

    /// Only when upgrading from >= "0.5.0" across into "0.6.0".
    #[once("0.5.0" => "0.6.0")]
    fn repair_settings(_ctx: &AppMigrationContext) -> Result<()> {
        // ...
        Ok(())
    }

    /// Not executed locally: stays pending until an external runner (e.g. a
    /// WebView frontend) reports completion. Keep the body empty.
    #[once("0.5.2", delegate)]
    fn webview_side_action() {}
}
```

The macro appends a hidden `registry()` inside the module, returning an `AppMigrationSet`. Eligibility is "the upgrade crossed into the action's version": `previous < since <= current`, with `previous` read from a JSON-file ledger (`AppMigrationStore`). A fresh install (no previous version) never migrates; a `from` version adds an origin floor.

Successful actions are recorded in the ledger and never run again. A failing action is not recorded and retries on the next launch — keep actions idempotent. `delegate` actions are listed by `set.delegated_pending(&store)` for an external executor, which reports back via `set.mark_completed(&mut store, id)`.

```rust
let set = app_migrations_registry::registry();
let mut store = AppMigrationStore::load(config_dir.join("app-migrations.json"))?;
store.seed_last_run_version(previous_hint); // only fills a fresh ledger
let run = set.run_immediate(&mut store); // executes + records local actions
let pending = set.delegated_pending(&store); // hand to the external runner
```

## TODO

- [x] Support `specta` for TypeScript type export.
- [x] Support `yuuka` (via serde-based interop layer).
- [x] Version field can use crate version automatically.
- [x] Generate migration code automatically from the git history.
- [x] Scaffold app-version one-time actions (`#[app_migrations]` + `#[once]`).

## Yuuka Interoperability

Hifumi provides a serde-based interop layer with [yuuka](https://github.com/celestia-island/yuuka). Since yuuka uses procedural macros (`derive_struct!`) while hifumi uses attribute macros (`#[version]`), deep integration is challenging. Instead, we provide utility functions for converting between the two formats:

```rust
use yuuka::derive_struct;
use hifumi::version;
use hifumi_e2e::yuuka_interop::{yuuka_to_hifumi_with_version, hifumi_to_yuuka};

// Define a yuuka config structure
derive_struct!(
    #[derive(serde::Serialize, serde::Deserialize)]
    pub YuukaConfig {
        name: String,
        value: i32,
    }
);

// Define a versioned hifumi structure with the same fields
#[version("0.1")]
#[derive(Debug, Clone, PartialEq)]
struct HifumiConfig {
    name: String,
    value: i32,
}

// Convert yuuka -> hifumi
let yuuka_cfg = YuukaConfig { name: "test".into(), value: 42 };
let hifumi_cfg: HifumiConfig = yuuka_to_hifumi_with_version(&yuuka_cfg, "0.1")?;

// Convert hifumi -> yuuka
let back: YuukaConfig = hifumi_to_yuuka(&hifumi_cfg)?;
```

## CLI Tool

Install the CLI tool:

```bash
cargo install hifumi-cli
```

### Analyze struct changes

```bash
hifumi-cli analyze -f src/models.rs -s MyStruct --from HEAD~1 --to HEAD
```

### Generate migration code

```bash
hifumi-cli generate -f src/models.rs -s MyStruct \
  --from-version "0.1" --to-version "0.2" \
  --from-commit HEAD~1 --to-commit HEAD
```

This will output migration code like:

```rust
#[migration("0.1" => "0.2" {
    + new_field: String,
    - old_field: i32,
    renamed_from => renamed_to: bool,
})]
```
