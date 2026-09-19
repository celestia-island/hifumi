import "./celestia-devtools.just"

set shell := ["bash", "-c"]
# Windows: PowerShell (the 5.1 floor ships with every Windows; pwsh 7 is
# NOT assumed). Linewise recipes must stay PS-5.1-safe: no `&&` chains,
# `cd X; cmd` instead of `cd X && cmd`. Bash-only recipes use
# [script('bash')] and need Git Bash (or WSL) when actually run.
set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command", "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; $PSDefaultParameterValues['*:Encoding']='utf8';"]
# `set lists` enables which() (used by the imported celestia-devtools.just);
# `set unstable` gates it.
set unstable
set lists

default:
    @just --list

fmt:
    just fmt-toml
    just fmt-markdown .
    cargo +nightly fmt --all
    cargo clippy -- -D warnings

fmt-check:
    just fmt-markdown . --check
    cargo +nightly fmt --all -- --check --unstable-features

check:
    cargo check

test:
    cargo test

build:
    just cache-guard
    cargo build

clean:
    cargo clean

ci: fmt-check check test
