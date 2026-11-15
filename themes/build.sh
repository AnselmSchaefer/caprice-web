#!/bin/bash
# Builds each theme program in themes/ as the writer's tab will (THEME_PROGRAMS_PLAN.md, step 4):
# rustc 1.99.1, the SDK as one rlib, the theme as one file, with the flags Caprice fixes, the
# output always named theme.wasm (wasm-ld writes the name into the module). Into
# tests/fixtures/<theme>.wasm, which the tests run: built again with the same rustc, a theme gives
# the same bytes. Needs `rustup toolchain install 1.99.1 -t wasm32-unknown-unknown`.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
out=$(mktemp -d "${TMPDIR:-/tmp}/themes.XXXXXX")
trap 'rm -rf "$out"' EXIT
fixed=(--edition 2024 --target wasm32-unknown-unknown -O -C codegen-units=1 -C panic=abort -C strip=debuginfo
       --remap-path-prefix "$PWD/theme-sdk=/sdk" --remap-path-prefix "$PWD/themes=/themes")
rustc +1.99.1 "${fixed[@]}" --crate-type rlib --crate-name theme_sdk theme-sdk/src/lib.rs -o "$out/libtheme_sdk.rlib"
for dir in themes/*/; do
    [ -f "$dir/theme.rs" ] || continue
    name=$(basename "$dir")
    mkdir -p "$out/$name"
    rustc +1.99.1 "${fixed[@]}" --crate-type cdylib --crate-name theme -C link-arg=--max-memory=33554432 \
        --extern theme_sdk="$out/libtheme_sdk.rlib" "$dir/theme.rs" -o "$out/$name/theme.wasm"
    cp "$out/$name/theme.wasm" "tests/fixtures/$name.wasm"
    echo "tests/fixtures/$name.wasm: $(wc -c < "tests/fixtures/$name.wasm") bytes"
done
