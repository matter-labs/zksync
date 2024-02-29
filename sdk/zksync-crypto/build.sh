#!/bin/bash

set -e

BG_ASM=dist/zksync-crypto-bundler_bg_asm.js
ASM=dist/zksync-crypto-bundler_asm.js

which wasm-pack || cargo install --version 0.10.1 wasm-pack #Dec 16th update to wasm-pack (v0.10.2) breaks zk init

# pack for bundler (!note this version is used in the pkg.browser field)
wasm-pack build --release --target=bundler --out-name=zksync-crypto-bundler --out-dir=dist
# pack for browser
wasm-pack build --release --target=web --out-name=zksync-crypto-web --out-dir=web-dist
# pack for node.js
wasm-pack build --release --target=nodejs --out-name=zksync-crypto-node --out-dir=node-dist

# Merge dist folders. wasm-pack removes out-dir before it starts a new build
mv web-dist/* dist/
mv node-dist/* dist/
rm -rf web-dist node-dist
rm dist/package.json dist/.gitignore

if [ "$CI" == "1" ]; then
    exit 0
fi

# convert the bundler build into JS in case the environment doesn't support WebAssembly
../build_binaryen.sh
../binaryen/bin/wasm2js ./dist/zksync-crypto-bundler_bg.wasm -o $BG_ASM

# save another copy for bg_asm import
# note that due to the behavior of wasm-pack we copy the different file:
# for a bundler build it extracts the content of .js file into _bg.js,
# we fix it ourselves
cp ./dist/zksync-crypto-bundler_bg.js $ASM

# fix imports for asm
sed -i.backup "s/^import.*/\
let wasm = require('.\/zksync-crypto-bundler_bg_asm.js');/" $ASM
sed -i.backup "s/\_bg.js/_asm\.js/g" $BG_ASM

rm dist/*.backup

# this is again related to how wasm-pack works
echo -e "\nwasm.__wbindgen_start();\n" >> $ASM
