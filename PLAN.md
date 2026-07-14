# hifumi — Maintenance Notes

> Created 2026-07-10 during a routine maintenance sweep.

## Refresh log 2026-07-14

- **当前分支**：`dev` · 领先 `origin/dev` 0 commits · 工作区干净
- **最近提交**：`🔧 Pin script recipes to the resolved Git Bash to survive WSL shadowing.` (`a10afd3`)
- **未提交改动**：无
- **后续动作**：
  1. 继续推进 hifumi 的「license metadata mismatch」维护决策（SySL 1.0 LICENSE 文本 vs `packages/macros` / `packages/types` 的 `license = "Apache-2.0"`）：由 maintainer 拍板二选一后，在 `dev` 分支上统一 `Cargo.toml` 的 `license` 字段与 `LICENSE.sha256` 校验。
  2. 跟进跨仓 `[patch]` 收敛到 `~/.cargo/config.toml`（见 `entelecheia/PLAN.md` §6 跨仓依赖约定）后，hifumi 各子 crate 的内联 `[patch.*]` 是否需移除；同步在顶层 `patches/` 长期方案中登记。
  3. 验证 `🔧 Pin script recipes` 提交后，hifumi 的 just 配方在 WSL 环境下使用解析到的 Git Bash 路径执行正常，覆盖 `just lint` / `just test` / `just package` 三条常用路径。
- **跨仓依赖**：上游依据 → `entelecheia/PLAN.md`；hifumi 是 entelecheia 生态中负责 crates.io 元数据/构建脚本的工具型子仓，与 aris、aoba、shirabe 等 Rust sibling 仓共享 just 配方风格。

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
