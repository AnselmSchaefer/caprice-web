#!/bin/bash
# Build Caprice for the browser into web/dist, with its headers (web/_headers, and the CSP). Threads need nightly Rust and its std built with
# atomics (see .cargo/config.toml), for this build only.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
# A nightly fixed by date, so a build is the same here and in CI.
export RUSTUP_TOOLCHAIN="${CAPRICE_NIGHTLY:-nightly-2026-10-23}"
export CARGO_UNSTABLE_BUILD_STD=std,panic_abort
trunk build --release --offline "$@"
# The fonts into a folder named for their contents, which the page names (its "fonts/", in the meta
# the app reads and the fonts it preloads): kept for good by the browser (web/_headers), so a reader
# who has been here before never waits for them, and a new font is a new name (R21).
fonts=$(cat web/fonts/* | sha256sum | cut -c1-16)
mkdir web/dist/fonts/"$fonts"
find web/dist/fonts -maxdepth 1 -type f -exec mv -t web/dist/fonts/"$fonts" {} +
sed -i "s#\"fonts/#\"fonts/$fonts/#g" web/dist/index.html
grep -q "\"fonts/$fonts/LiberationSerif-Regular.ttf\"" web/dist/index.html || { echo "the page names no fonts in fonts/$fonts"; exit 1; }
# The compiler for theme programs (R36), into web/dist/toolchain and web/dist/compiler, from the
# release toolchain/release.json pins (`node toolchain/fetch.mjs` fetches it; pack.mjs says where it
# looks). CAPRICE_TOOLCHAIN=0 leaves it out: the build then cannot make themes, and says so where
# one is made.
if [ "${CAPRICE_TOOLCHAIN:-1}" != 0 ]; then
    node toolchain/pack.mjs web/dist
fi
# The Content-Security-Policy names the inline scripts of this very build (web/csp.mjs).
node web/csp.mjs web/dist
