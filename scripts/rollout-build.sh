#!/usr/bin/env bash
set -euo pipefail
# Local artifact build only. Does not deploy, sign or change chain state.
cd "$(dirname "$0")/.."
optimizer_image=cosmwasm/workspace-optimizer@sha256:b9c92b2900b7ebaab3499203615c1b8589592bc557355ed3432e48851ffde69e
cargo +1.81.0 build -p cw20-mintable -p cw20-security-controller --lib --release --target wasm32-unknown-unknown --locked
mkdir -p artifacts
for name in cw20_mintable cw20_security_controller; do
  docker run --rm --user "$(id -u):$(id -g)" --entrypoint wasm-opt \
    -v "$PWD/target/wasm32-unknown-unknown/release:/input:ro" \
    -v "$PWD/artifacts:/artifacts" "$optimizer_image" \
    -Os --signext-lowering "/input/$name.wasm" -o "/artifacts/$name.wasm"
  wasm-tools validate "artifacts/$name.wasm"
done
# Install the matching checker separately if unavailable: cargo +1.81.0 install cosmwasm-check --version 1.5.11 --locked
checker=${COSMWASM_CHECK:-cosmwasm-check}
[[ "$("$checker" --version)" == "Contract checking 1.5.11" ]]
"$checker" artifacts/cw20_mintable.wasm artifacts/cw20_security_controller.wasm
sha256sum artifacts/cw20_mintable.wasm artifacts/cw20_security_controller.wasm
