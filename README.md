<img src="splash.png" alt="hifumi" />

![Crates.io License](https://img.shields.io/crates/l/hifumi)
[![Crates.io Version](https://img.shields.io/crates/v/hifumi)](https://docs.rs/hifumi)
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

## TODO

- [x] Support `specta` for TypeScript type export.
- [ ] Support `yuuka` (requires design for macro interop).
- [x] Version field can use crate version automatically.
- [x] Generate migration code automatically from the git history.

## CLI Tool

Install the CLI tool:

```bash
cargo install hifumi-cli
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
