# hifumi — Maintenance Notes

> Created 2026-07-10 during a routine maintenance sweep.

## Open issue: license metadata mismatch (needs maintainer decision)

The repository ships a **Synthetic Source License (SySL) 1.0** text in the
root `LICENSE` file, but every published crate declares the SPDX license as
**Apache-2.0** in its `Cargo.toml`:

| File | Declared license |
|------|------------------|
| `LICENSE` (file text) | SySL 1.0 |
| `packages/macros/Cargo.toml` | `license = "Apache-2.0"` |
| `packages/types/Cargo.toml` | `license = "Apache-2.0"` |
| `packages/cli/Cargo.toml` | `license = "Apache-2.0"` |

This means the crates.io metadata advertises Apache-2.0 while the actual
license file is SySL — a real conflict for downstream users relying on the
SPDX expression.

### Why this was not auto-fixed

SySL 1.0 is not a standard SPDX identifier, so crates.io does not accept it
as a plain `license = "SySL-1.0"` value. Sibling crates in the ecosystem
(e.g. `hikari`) use `license-file = "LICENSE"` to publish SySL-licensed
crates. Switching hifumi to that form changes the published license
metadata and likely warrants a new semver bump on each crate, so it is left
for the maintainer.

### Suggested resolution (pick one)

1. **The crates really are Apache-2.0**: replace the root `LICENSE` with the
   Apache-2.0 text (and add the SySL text only where intended), and add a
   license badge to the README.
2. **The crates are SySL**: change each `Cargo.toml` to
   `license-file = "LICENSE"` (dropping the `license = "Apache-2.0"` line)
   and bump the crate versions, mirroring `hikari`.

## Done during this sweep

- Renamed the stale all-checked `## TODO` section to `## Features`.
- Added `*.swp` / `.DS_Store` / `Thumbs.db` to `.gitignore`.
