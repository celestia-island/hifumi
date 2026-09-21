# celestia-devtools common.just — shared build/devtool recipes.
#
# This file is NOT committed to your repo. It is staged on demand into .just/
# (gitignored) by `just fetch` in your justfile, then pulled in via an optional
# import: `import? "./.just/celestia-devtools.just"`. The `?` lets a fresh clone
# parse the justfile before fetch, so your own recipes always work.
#
# Stage or refresh it with:  just fetch
# Version: 0.7.0
#
# All recipes and variables are overridable — redefine anything after the
# `import?` line in your justfile to customize it.
#
# This file is intentionally free of `set` directives (shell, windows-shell,
# unstable) so it composes cleanly with each repo's own header.
#
# ── SHELL DOCTRINE (0.7.0) ────────────────────────────────────────────────────
# Every recipe below is a LINEWISE, shell-neutral single command: valid under
# POSIX sh AND Windows PowerShell 5.1 alike. No bash, no `[script]` bodies, no
# shebangs, no .sh/.ps1 helper scripts — multi-step logic lives in the
# celestia-devtools CLI (pure Python) instead. The only Windows-side runtime
# requirements are PowerShell + `python` + `curl`, all inbox or standard.
# The former `.just/git-bash-interop.just` / `set script-interpreter` crutch
# and the `windows-shell-check` WSL-hijack diagnostic are retired with it.
#
# The importing justfile MUST set (see any sibling repo for the exact bytes):
#     set shell := ["bash", "-c"]
#     set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile",
#                           "-Command", "<UTF-8 prelude>"]
# and keep its own linewise recipes inside the same two-shell intersection:
# one command per line, no `&&`/`||`/`$()`/line-continuations. Bash-syntax
# logic belongs in the CLI, or in a `[script('python')]` body at most.

# ── Tool resolution ───────────────────────────────────────────────────────────

# On Windows prefer `python` — `python3` is often the broken WindowsApps Store
# stub that exits 49. On Unix, `python3` is the norm.
python_cmd := if os_family() == "windows" {
    if which("python") != "" { "python" } else { "python3" }
} else {
    if which("python3") != "" { "python3" } else { "python" }
}

# Resolve the celestia-devtools CLI: installed entry point, or module fallback.
_devtools := if which("celestia-devtools") != "" { "celestia-devtools" } else { python_cmd + " -m celestia_devtools" }

# ── Bootstrap ─────────────────────────────────────────────────────────────────

# Ensure celestia-devtools is installed. Run once on a fresh checkout.
# Override the install source with $CELESTIA_DEVTOOLS_INSTALL (e.g. a local path).
# The conditional install is a `python -c` expression on purpose: a single
# command that both sh and PowerShell can run verbatim — the old three-way
# `||` pipeline was bash-only and crashed under PowerShell.
ensure:
    {{python_cmd}} -c "import importlib.util as u, os, subprocess as s, sys; s.call([sys.executable, '-m', 'pip', 'install', os.environ.get('CELESTIA_DEVTOOLS_INSTALL') or 'git+https://github.com/celestia-island/celestia-devtools.git']) if u.find_spec('celestia_devtools') is None else None"

# ── Build helper ──────────────────────────────────────────────────────────────

# Shared build dispatcher: runs a prerequisite, then picks dev vs release cmd.
#
#   just _build ":" "cargo build" "cargo build --release" {{FLAGS}}
#   just _build "just cache-guard" "cargo build" "cargo build --release" --dev
#
# Flags (consumed by the `build-dispatch` CLI command):
#   --dev    use the dev command instead of release
#   --clean  cargo clean before building
_build always_pre dcmd rcmd *FLAGS='':
    {{ _devtools }} build-dispatch {{always_pre}} {{dcmd}} {{rcmd}} {{FLAGS}}

# ── Cache management ──────────────────────────────────────────────────────────

# target/ cache guard:
#   free-disk floor < 10 GiB → cargo clean (instant check, every run)
#   target ≥ 40 GiB → cargo sweep --time 7 (needs cargo-sweep; cached du)
# Tune via flags or CARGO_CACHE_GUARD_* env vars. Run before `just build`.
cache-guard *ARGS='':
    {{ _devtools }} cache-guard . {{ARGS}}

# Backwards-compatible alias of cache-guard.
sweep-guard min_free="10":
    {{ _devtools }} cache-guard . --min-free-gib {{min_free}}

# Manually remove target/**/incremental/ (keeps compiled dep artifacts).
clean-incremental:
    {{ _devtools }} cache-guard . --clean-incremental

# ── Formatting ────────────────────────────────────────────────────────────────

# Sort Cargo.toml [workspace.dependencies] entries (internal → celestia → external).
# Call inside your fmt/fmt-check recipe.
fmt-toml *ARGS='':
    {{ _devtools }} toml-sort . {{ARGS}}

# Format Markdown docs (stdlib-only; call inside your fmt/fmt-check recipe).
# Does NOT touch Rust or JS — compose with cargo fmt / pnpm lint yourself.
fmt-markdown *ARGS='':
    {{ _devtools }} format-markdown . {{ARGS}}

# Lint and fix decorative ==== comment separators and merge-conflict markers.
# Run with --fix to apply changes in-place.
lint-separators *FLAGS='':
    {{ _devtools }} lint-separators . {{FLAGS}}

# Vendor the five org protocol docs (license/cla/code-of-conduct/security/
# contributing) from docs.celestia.world into packages/webui/.generated/
# protocols as per-(type, lang) vendor markdown plus a meta.ts manifest.
# Resolves the docs checkout via $DOCS_CELESTIA_WORLD_ROOT, the sibling
# ../docs.celestia.world, or a shallow clone into target/docs-celestia-shared
# (the owned clone self-refreshes to the remote head on every run).
protocol-bundle *FLAGS='':
    {{ _devtools }} protocol-bundle {{FLAGS}}

# ── Commit message lint ──────────────────────────────────────────────────────

# Validate a commit message against the org gitmoji convention.
# Pass the commit-msg file path (git passes $1 to the hook).
commit-msg-lint FILE:
    {{ _devtools }} commit-msg-lint check {{ FILE }}

# Install (or refresh) the org commit-msg hook into .git/hooks/.
# Normally triggered automatically by `celestia-devtools init`; call this
# directly to reinstall after a force-override or to install into a fresh
# checkout where init has already vendored the justfile.
commit-msg-hook-install *ARGS='':
    {{ _devtools }} hook install {{ ARGS }}

# ── P0 ledger gate ───────────────────────────────────────────────────────────

# Fail when this repo is covered by an unresolved P0 in the org ledger
# (shipped with celestia-devtools as lint/p0_ledger.toml; override with
# $CELESTIA_P0_LEDGER or a repo-root p0-ledger.toml).
#
# An open entry blocks the repo until it is closed (closed_by = the PR that did
# it) or explicitly acknowledged by a human — `--ack P0-X`, $CELESTIA_P0_ACK,
# or a `P0-ACK: P0-X` line in the PR body (pass it with --pr-body-file). The
# gate echoes every acknowledgment; it never passes silently.
#
#   just p0-gate                 check this repo
#   just p0-gate --list          print the whole ledger, grouped by status
#   just p0-gate --ack P0-A      acknowledge one open item (recorded in the log)
p0-gate *ARGS='':
    {{ _devtools }} p0-gate {{ARGS}}

# ── Upstream fork sync ───────────────────────────────────────────────────────
#
# For repos that track an upstream (kei ← asterinas, aris ← servo, …). Fetches
# the latest upstream commits so a human can then merge or rebase deliberately
# — this recipe never auto-merges.
#
# Prereqs: an `upstream` git remote. If absent it is added from $UPSTREAM_URL
# (set it in your .env or shell); otherwise the command errors with guidance.

upstream-sync:
    {{ _devtools }} upstream-sync

# ── Dependencies ──────────────────────────────────────────────────────────────

# Pre-stage all deps for offline builds (cargo fetch + pnpm/npm/yarn install).
# Run once after cloning (needs network); subsequent builds run fully offline.
prefetch:
    {{ _devtools }} prefetch .

# ── Cross-compilation ─────────────────────────────────────────────────────────

# Check and auto-install cross-compilation prerequisites (rustup target +
# ziglang + cargo-zigbuild). Mirror selection auto-detected (TUNA for CN).
cross-check target='aarch64-unknown-linux-gnu' features='full':
    {{ _devtools }} check-cross-deps --target {{target}} --features {{features}}

# ── Crate location ────────────────────────────────────────────────────────────

# Locate a celestia-island crate checkout (env → cargo [patch] → sibling → clone).
locate crate:
    {{ _devtools }} locate --crate {{crate}}

# Print the local path of a sibling repo, honouring $<NAME>_ROOT or $<NAME>_REPO
# (the kei/aris convention, e.g. $ARIS_REPO) before falling back to a sibling
# checkout. Exits non-zero if the repo cannot be found, so it is safe to use as
#   ARIS := `just link-sibling aris`
# in your own recipes.
link-sibling crate:
    {{ _devtools }} locate --crate {{crate}}

# ── File-watch dev server (malkuth) ──────────────────────────────────────────
#
# malkuth (celestia-island/malkuth) wraps any command with file-watching +
# auto-restart: notify v8 recursive watch, 400 ms debounce, kill+respawn.
#
# RECOMMENDED PATTERN: do NOT call `just dev-watch` from your repo's `dev`
# recipe — that forks a child `just` process. Make your `dev` recipe call
# malkuth directly (linewise single command), or the CLI's supervision.
#
# Override the malkuth binary location with $MALKUTH_BIN or $MALKUTH_ROOT.

# Resolve the malkuth binary path (env → PATH → sibling layout). Kept as a
# just variable for repo recipes that embed it; `dev-watch` resolves it the
# same way inside the CLI.
malkuth_bin := if env("MALKUTH_BIN", "") != "" {
    env("MALKUTH_BIN", "")
} else if env("MALKUTH_ROOT", "") != "" {
    env("MALKUTH_ROOT", "") + if os_family() == "windows" { "/target/release/malkuth.exe" } else { "/target/release/malkuth" }
} else if which("malkuth") != "" {
    "malkuth"
} else {
    "../malkuth/target/release/" + if os_family() == "windows" { "malkuth.exe" } else { "malkuth" }
}

# Always-valid sibling-repo fallback path (used when malkuth_bin resolves to a
# bare name that isn't actually on PATH at runtime).
malkuth_bin_rel := "../malkuth/target/release/" + if os_family() == "windows" { "malkuth.exe" } else { "malkuth" }

# Supervise a command with malkuth: `just dev-watch src -- cargo run`.
# Args split at `--`: watch paths before, supervised command after.
dev-watch *ARGS:
    {{ _devtools }} dev-watch {{ARGS}}

# Auto-register celestia-island git deps as cargo [patch] entries in
# ~/.cargo/config.toml. Runs `cargo metadata` to discover which celestia crates
# the current repo ACTUALLY depends on, then patches only those to their local
# checkouts. Scan directory: --scan-dir > $CELESTIA_ROOT > parent of cwd.
#
#   just register-patches                        scan + write
#   just register-patches --dry-run              preview only
#   just register-patches --scan-dir ../celestia pin scan directory
register-patches *ARGS='':
    {{ _devtools }} register-patches {{ARGS}}

# ── Git worktree management ───────────────────────────────────────────────────
#
# Linked working trees that share one .git directory. Each worktree is an
# independent checkout at ../<repo>-<name> with its own branch, index, and
# files — but shares objects/refs with the main repo. Rationale:
#
# 1. Different PRs need different Cargo.toml [patch] entries (A→B#feature vs
#    A→B#master), and you cannot express that in a single working directory.
# 2. In a monorepo-with-many-sibling-repos setup (celestia-island), each
#    worktree gets its own topological patch graph.
# 3. No rebuild storms: switching a worktree is `cd`, not `git checkout`.
#
# Recipes:
#   just worktree-create pr-42 dev     checkout ../repo-pr-42 from dev
#   just worktree-list                  list all worktrees for this repo
#   just worktree-remove pr-42          delete worktree + prune stale refs

# Create a new worktree at ../<repo>-<name> from <base> (branch or commit).
# After creation registers cargo patches for the new directory.
worktree-create name base='dev':
    {{ _devtools }} worktree-create {{name}} {{base}}

# List all worktrees for this repo.
worktree-list:
    git worktree list

# Remove a worktree and prune its metadata.
worktree-remove name:
    {{ _devtools }} worktree-remove {{name}}

# ── WSL2 dev environment ──────────────────────────────────────────────────────
#
# The celestia ecosystem shares ONE persistent WSL2 distro (default
# "celestia-dev") provisioned with rust + just + docker, so the global cargo
# registry cache is reused across aris / kei / entelecheia / shittim-chest
# instead of re-fetching every dependency per repo. Same model as Docker
# Desktop's managed WSL2 backend.
#
#   just wsl-ensure              create/verify (idempotent — safe to re-run)
#   just wsl-ensure --reset      destroy + recreate (clears ALL caches)
#   just wsl-ensure --no-docker  skip docker (use Docker Desktop integration)
#   just wsl-ensure --force      re-provision even if distro already exists
wsl-ensure *ARGS='':
    {{ _devtools }} wsl-ensure {{ARGS}}

# Run a command inside the celestia-dev distro (cwd = this repo's root).
# Example: just wsl-run cargo build --release
# The `bash -lc` executes INSIDE the Linux distro — that is the distro's own
# shell on the Linux side of the boundary, not a Windows-host dependency.
wsl-run *CMD='echo usage: just wsl-run <command>':
    wsl -d celestia-dev --cd "{{ justfile_directory() }}" -- bash -lc "{{ CMD }}"

# Drop into an interactive shell in the distro at the repo root.
wsl-shell:
    wsl -d celestia-dev --cd "{{ justfile_directory() }}"

# ── QEMU cross-architecture testing ──────────────────────────────────────────
#
# Ensure QEMU system emulators are installed for multi-arch testing
# (aarch64, x86_64, riscv64). Cross-platform: detects Windows QEMU, Linux
# apt/conda, macOS Homebrew — no root password required when possible.
#
#   just qemu-ensure              check/install all archs
#   just qemu-ensure --check      report status only (no install)
#   just qemu-ensure aarch64      ensure only aarch64
qemu-ensure *ARGS='':
    {{ _devtools }} qemu-ensure {{ARGS}}

# ── Environment preflight ────────────────────────────────────────────────────
#
# Verify required dev tools are present BEFORE a facility (pglite, build,
# cross-compile, …) actually needs them, so the user gets one actionable
# hint ("missing node → run just wsl-ensure") instead of a stack trace.
#
#   just preflight                 check ALL known tools
#   just preflight node npm        check only node + npm
#   just preflight -- --quiet      exit code only (for scripts)
preflight *REQS='':
    {{ _devtools }} preflight {{REQS}}

# ── Shared temporary PGlite ──────────────────────────────────────────────────
#
# Spawn a real embedded Postgres (PGlite WASM + pgvector) over the PG wire
# protocol, shared between entelecheia and shittim-chest for tests / mock
# backends. Zero Docker / system-postgres; data is in-memory unless --persist.
#
#   just pglite start              spawn, print DATABASE_URL (auto free port)
#   just pglite start --port 5433  pin a port
#   just pglite start --persist ./pgdata   keep data across restarts
#   just pglite stop               stop the recorded instance
#   just pglite status             is one running? prints JSON
pglite *ARGS='':
    {{ _devtools }} pglite {{ARGS}}

# ── npx precompiled distribution ─────────────────────────────────────────────
#
# Turn a cross-compiled Rust binary into a publishable npm package so users run
# it via `npx @celestia-island/<pkg>` with no Rust toolchain required. The
# generator writes a two-layer layout under ./dist: a root launcher
# (@scope/pkg) with optionalDependencies on per-platform subpackages, plus one
# subpackage per target carrying only that binary.
#
# Local dry-run (verify the staged packages without publishing):
#
#   just npm-dist shirabe                                        # reassemble root from existing dist/
#   just npm-dist shirabe 0.1.0 path/to/shirabe x86_64-unknown-linux-gnu
#
# Per-platform subpackages are staged from CI (one matrix job per target),
# then a final job reassembles the root. Actual `npm publish` runs in CI; this
# recipe stages + runs `npm pack --dry-run` so you can inspect the tarball.
npm-dist name version='' binary='' target='':
    {{ _devtools }} npm-dist --name {{name}} --out-dir dist --pack-dry-run {{ if version != "" { "--version " + version } else { "" } }} {{ if binary != "" { "--binary " + binary + " --rust-target " + target } else { "" } }}

# Publish the staged packages to npm (requires NPM_TOKEN in the environment).
# Intended for CI; prompts interactively for the OTP if 2FA is on.
# Override the tag with NPM_DIST_TAG (default: latest).
npm-release *PKGS='':
    {{ _devtools }} npm-release {{PKGS}}

# Overlay local @celestia-island sibling checkouts into node_modules via
# symlinks for local development (repo manifests stay registry-only).
#   just link-npm-siblings             # apply/refresh links
#   just link-npm-siblings --status    # inspect current state
#   just link-npm-siblings --remove    # back to pure registry resolution
# Run after every `pnpm install` — installs restore registry resolution.
link-npm-siblings *ARGS='':
    {{ _devtools }} link-npm-siblings {{ARGS}}

# ── Vite frontend build — no dev server, no HMR, no overlay noise ─────────────
#
# celestia frontends (shittim-chest, hikari, tairitsu, …) use vite build +
# a plain static file server. No vite dev, no HMR WebSocket, no error overlay.
# Rebuilds are triggered by malkuth (or any file watcher) — the watcher owns
# the terminal output, so build logs stay clean and under our control.
#
#   just vite-build               one-shot production build (vite build)
#   just vite-build -- --mode mock    build with VITE_MODE=mock (env passthrough)
#   just vite-serve                serve dist/ on port 5173 with python http.server
#   just vite-serve --port 3000    serve on a custom port
#   just vite-dev                  build → serve → watch src/ for rebuilds (malkuth)

# One-shot build. Pass extra flags or env vars after `--`.
vite-build *ARGS='':
    {{ _devtools }} vite-build {{ARGS}}

# Serve the built dist/ output with python http.server (zero JS deps, no npm).
vite-serve *ARGS='':
    {{ _devtools }} vite-serve {{ARGS}}

# Full dev loop: build once, serve, then watch src/ with malkuth. The static
# server stays alive and picks up the rebuilt dist/ automatically.
vite-dev *ARGS='':
    {{ _devtools }} vite-dev {{ARGS}}

# ── Mock server orchestration ─────────────────────────────────────────

# Start all mock servers (arona → entelecheia → shittim-chest).
# Each repo must be checked out as a sibling directory.
# Press Ctrl+C to stop all.
mock-start:
    {{ _devtools }} mock-start

# Stop all running mock servers.
mock-stop:
    {{ _devtools }} mock-stop

# Show status of all registered mock servers.
mock-status:
    {{ _devtools }} mock-status
