import "./celestia-devtools.just"

set shell := ["bash", "-c"]
# `set windows-shell` only governs linewise (non-shebang) recipes on Windows.
# Shebang recipes bypass it and force `just` to call `cygpath` to translate the
# interpreter path — which Git for Windows keeps off PATH, so they die with
# "could not find cygpath executable". To avoid that, every multi-line recipe
# below uses the `[script('bash')]` attribute instead of a `#!` shebang:
# `[script]` resolves the interpreter via PATH (PATHEXT-aware) and never calls
# cygpath. See casey/just#2828 and the just manual (Script Recipes).
set windows-shell := ["bash.exe", "-c"]
# `set lists` enables which() (used by the imported celestia-devtools.just);
# `set unstable` gates it.
set unstable
set lists

default:
    @just --list

fmt:
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
