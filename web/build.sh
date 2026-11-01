#!/bin/bash
# Build Caprice for the browser into web/dist, with its headers (web/_headers, and the CSP). Threads need nightly Rust and its std built with
# atomics (see .cargo/config.toml), for this build only.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=nightly
export CARGO_UNSTABLE_BUILD_STD=std,panic_abort
trunk build --release --offline "$@"
# The Content-Security-Policy names the inline scripts of this very build (web/csp.mjs).
node web/csp.mjs web/dist
