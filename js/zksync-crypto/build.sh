#!/bin/bash

set -e

which wasm-pack || cargo install wasm-pack
wasm-pack build --release --out-name=web --out-dir=dist
wasm-pack build --release --target=nodejs --out-name=node --out-dir=dist
rm dist/package.json
