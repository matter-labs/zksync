#!/bin/bash

set -e

ASM=dist/zksync-crypto-bundler_bg_asm.js

which wasm-pack || cargo install wasm-pack

# pack for bundler (!note this verion is used in the pkg.browser field)
wasm-pack build --release --target=bundler --out-name=zksync-crypto-bundler --out-dir=dist
# pack for browser
wasm-pack build --release --target=web --out-name=zksync-crypto-web --out-dir=dist
# pack for node.js
wasm-pack build --release --target=nodejs --out-name=zksync-crypto-node --out-dir=dist

rm dist/package.json dist/.gitignore

if [ "$CI" == "1" ]; then
    exit 0
fi

# convert the bundler build into JS in case the environment doesn't support WebAssembly
../build_binaryen.sh
../binaryen/bin/wasm2js ./dist/zksync-crypto-bundler_bg.wasm -o $ASM

# save another copy for bg_asm import
cp ./dist/zksync-crypto-bundler.js ./dist/zksync-crypto-bundler_asm.js

# fix imports for asm
sed -i.backup "s/^import.*/\
let wasm = require('.\/zksync-crypto-bundler_bg_asm.js');/" ./dist/zksync-crypto-bundler_asm.js
sed -i.backup "s/\.js/_asm\.js/g" $ASM

rm dist/*.backup
