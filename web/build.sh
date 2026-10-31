#!/bin/bash
# Build Caprice for the browser into web/dist. Threads need nightly Rust and its std built with
# atomics (see .cargo/config.toml), for this build only.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=nightly
export CARGO_UNSTABLE_BUILD_STD=std,panic_abort
trunk build --release --offline "$@"
