#!/usr/bin/env bash
# Copyright 2019-2020 @polkadot/wasm authors & contributors
# This software may be modified and distributed under the terms
# of the Apache-2.0 license. See the LICENSE file for details.

set -e

BGJ=dist/zksync_crypto_bg.js
SRC_WASM=dist/zksync_crypto.js
DEF=dist/zksync_crypto.d.ts
WSM=dist/zksync_crypto_bg.wasm
OPT=dist/zksync_crypto_opt.wasm
ASM=dist/zksync_crypto_asm.js

# which wasm-pack || cargo install wasm-pack
# which wasm-opt || cargo install wasm-opt

echo "*** Building package"

# cleanup old
echo "*** Cleaning old builds"
rm -rf ./dist ./pkg

# build new via nightly & wasm-pack
echo "*** Building WASM output"
rustup run nightly$NIGHTLY wasm-pack build --release --target nodejs
mv ./pkg ./dist

# optimise
echo "*** Optimising WASM output"
./../binaryen/bin/wasm-opt $WSM -Os -o $OPT -O

# convert wasm to base64 structure
echo "*** Packing WASM into base64"
node ./pack-wasm-base64.js

# build asmjs version from the input (optimised) WASM
echo "*** Building asm.js version"
./../binaryen/bin/wasm2js -Oz --output $ASM $OPT

# cleanup the generated asm, converting to cjs
sed -i -e '/import {/d' $ASM
echo "const imported = require('./zksync_crypto');
$(cat $ASM)" > $ASM
sed -i -e 's/{abort.*},memasmFunc/imported, memasmFunc/g' $ASM
sed -i -e 's/export var /module\.exports\./g' $ASM

# copy our package interfaces
echo "*** Copying package sources"
cp src/js/* dist/

echo "const crypto = require('crypto');
const { stringToU8a, u8aToString } = require('@polkadot/util');
const requires = { crypto };
$(cat $SRC_WASM)
" > $SRC_WASM

# whack comments
sed -i -e '/^\/\*\*/d' $SRC_WASM
sed -i -e '/^\*/d' $SRC_WASM
sed -i -e '/^\*\//d' $SRC_WASM

# we are swapping to a async interface for webpack support (wasm limits)
sed -i -e '/^wasm = require/d' $SRC_WASM

# We don't want inline requires
sed -i -e 's/ret = require(getStringFromWasm0(arg0, arg1));/ret = requires[getStringFromWasm0(arg0, arg1)];/g' $SRC_WASM

# this creates issues in both the browser and RN (@polkadot/util has a polyfill)
sed -i -e '/^const { TextEncoder } = require/d' $SRC_WASM
sed -i -e '/^let cachedTextEncoder = new /d' $SRC_WASM
sed -i -e 's/cachedTextEncoder\.encode/stringToU8a/g' $SRC_WASM

# this creates issues in both the browser and RN (@polkadot/util has a polyfill)
sed -i -e '/^const { TextDecoder } = require/d' $SRC_WASM
sed -i -e '/^let cachedTextDecoder = new/d' $SRC_WASM
sed -i -e 's/cachedTextDecoder\.decode/u8aToString/g' $SRC_WASM

# this is where we get the actual bg file
sed -i -e '/^const path = require/d' $SRC_WASM
sed -i -e '/^const bytes = require/d' $SRC_WASM
sed -i -e '/^const wasmModule =/d' $SRC_WASM
sed -i -e '/^const wasmInstance =/d' $SRC_WASM
sed -i -e '/^wasm = wasmInstance/d' $SRC_WASM

# construct our promise and add ready helpers (WASM)
echo "module.exports.abort = function () { throw new Error('abort'); };
const createPromise = require('./zksync_crypto_promise');
const wasmPromise = createPromise().catch(() => null);
module.exports.isReady = function () { return !!wasm; }
module.exports.waitReady = function () { return wasmPromise.then(() => !!wasm); }
wasmPromise.then((_wasm) => { wasm = _wasm });
" >> $SRC_WASM

# add extra methods to type definitions
echo "
export function isReady(): boolean;
export function waitReady(): Promise<boolean>;
" >> $DEF

rm dist/package.json dist/.gitignore