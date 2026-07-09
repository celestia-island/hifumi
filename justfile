import "./celestia-devtools.just"

set shell := ["bash", "-c"]
# On Windows just resolves recipe shebangs through the shell named here; without
# it just falls back to `cygpath`, which Git for Windows does not put on PATH.
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
