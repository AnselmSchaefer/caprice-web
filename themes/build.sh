#!/bin/bash
# Builds each theme program in themes/ as the writer's tab does (web/compiler):
# rustc 1.99.1, the SDK as one rlib, the theme as one file, with the flags Caprice fixes, the
# output always named theme.wasm (wasm-ld writes the name into the module). Into
# tests/fixtures/<theme>.wasm, which the tests run: built again with the same rustc, a theme gives
# the same bytes. Needs `rustup toolchain install 1.99.1 -t wasm32-unknown-unknown`.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
out=$(mktemp -d "${TMPDIR:-/tmp}/themes.XXXXXX")
trap 'rm -rf "$out"' EXIT
# Absolute paths, so the remapping applies: the SDK's files are /sdk/..., and every theme is
# /theme/theme.rs, wherever it was written, as the browser builds it (web/compiler/compiler.js).
# A path is in the module (where a panic would say it happened), so this is what makes the bytes
# the same. The flags are toolchain/pack.mjs's, which checks the fixtures against them.
fixed=(--edition 2024 --target wasm32-unknown-unknown -O -C codegen-units=1 -C panic=abort -C strip=debuginfo)
rustc +1.99.1 "${fixed[@]}" --crate-type rlib --crate-name theme_sdk --remap-path-prefix "$PWD/theme-sdk=/sdk" \
    "$PWD/theme-sdk/src/lib.rs" -o "$out/libtheme_sdk.rlib"
for dir in themes/*/; do
    [ -f "$dir/theme.rs" ] || continue
    name=$(basename "$dir")
    mkdir -p "$out/$name"
    rustc +1.99.1 --crate-name theme "${fixed[@]}" --crate-type cdylib -C link-arg=--max-memory=33554432 \
        --remap-path-prefix "$PWD/themes/$name=/theme" --extern theme_sdk="$out/libtheme_sdk.rlib" \
        "$PWD/themes/$name/theme.rs" -o "$out/$name/theme.wasm"
    cp "$out/$name/theme.wasm" "tests/fixtures/$name.wasm"
    echo "tests/fixtures/$name.wasm: $(wc -c < "tests/fixtures/$name.wasm") bytes"
done
