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

### Downgrade Writes

Migrations can declare a reverse direction with `back:` converters; `to_version_value` then writes any older version of the struct as JSON (`$version` tag included). Adds, copies, and pure renames are reversible for free; removes and renames with converters need an explicit `back` expression:

```rust
use hifumi::version;

#[version("0.3")]
#[derive(Debug, Clone, PartialEq)]
#[migration("0.2" => "0.3" {
    - legacy: bool { back: level < 2 },
})]
struct Config {
    name: String,
    level: i32,
}

let older: serde_json::Value = current.to_version_value("0.2")?;
```

### Forward Compatibility

`#[version("0.2", forward)]` opts a type into two-stage deserialization: data written by a **newer** version (unknown `$version`, unknown fields dropped) parses leniently — the known fields are kept — instead of being rejected. For types you cannot redefine, wrap fields or containers in [`Forward<T>`](packages/types/src/forward.rs) to bypass per value.

### Registries with Compile-Time Policies

Collect versioned types into a registry and let the compiler prove each type satisfies the policy it is registered under — a non-reversible type in `policy: backward` is a compile error, not a runtime surprise. `policy: full` requires every registered type to be both reversible and forward:

```rust
mod registry {
    hifumi::registry! {
        policy: full {
            FullDoc,
            LenientDoc,
        }
    }
}

let spec = registry::spec(); // type name, version chain, reversible, forward
```

### Baselines & Chain Compaction

`#[version("0.3", baseline = "0.2")]` marks versions below the baseline as extinct: their structs, `From` impls, and downgrade steps are no longer generated, and their JSON deserializes as an unknown variant. `hifumi compact` rewrites the source file in place to drop extinct migrations while preserving bytes elsewhere:

```bash
hifumi compact -f src/models.rs -s Config --baseline "0.2" --write
```

### Binary Wire Channel

`hifumi::wire` frames values as `b"HIFU"` + a u32 hash of `type@version` + a postcard payload of the **final** struct (no per-version enum on the wire). The macro generates a `WireCodec` impl next to every versioned struct:

```rust
let frame: Vec<u8> = hifumi::wire::to_wire(&doc, &Doc::__HIFUMI_META)?;
let doc: Doc = hifumi::wire::from_wire(&frame, &Doc::__HIFUMI_META)?;
```

## TODO

- [x] Support `specta` for TypeScript type export.
- [x] Export a forward-only TypeScript migration runner from Rust `#[version]` chains (`hifumi export-ts`).
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
cargo install hifumi
```

### Analyze struct changes

```bash
hifumi analyze -f src/models.rs -s MyStruct --from HEAD~1 --to HEAD
```

### Generate migration code

```bash
hifumi generate -f src/models.rs -s MyStruct \
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

### Export a TypeScript migration runner

`hifumi export-ts` reads every `#[version]` struct in a file and emits one self-contained forward-only TypeScript runner per struct (no dependencies). `migrate(doc, from, to)` upgrades a plain JSON object along the chain; downgrades throw. Ops with Rust converters, multi-source adds, type-crossing renames, and `PathBuf`-like types are refused loudly instead of exporting a lying runner:

```bash
hifumi export-ts -f src/models.rs --out migrations.ts
```

### Compact a version chain

Drops extinct migrations below a baseline from the source file in place (see [Baselines & Chain Compaction](#baselines--chain-compaction)):

```bash
hifumi compact -f src/models.rs -s Config --baseline "0.2" --write
```
