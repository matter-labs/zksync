#!/bin/bash

set -e

which wasm-pack || cargo install wasm-pack

# pack for bundler (!note this verion is used in the pkg.browser field)
wasm-pack build --release --target=bundler --out-name=zksync-crypto-bundler --out-dir=dist

# pack for browser
wasm-pack build --release --target=web --out-name=zksync-crypto-web --out-dir=dist

# pack for node.js
wasm-pack build --release --target=nodejs --out-name=zksync-crypto-node --out-dir=dist
rm dist/package.json
