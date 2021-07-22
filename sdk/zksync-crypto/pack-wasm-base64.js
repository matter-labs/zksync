// Copyright 2019-2020 @polkadot/wasm authors & contributors
// This software may be modified and distributed under the terms
// of the Apache-2.0 license. See the LICENSE file for details.

/* eslint-disable @typescript-eslint/no-var-requires */
const fs = require('fs');
const buffer = fs.readFileSync('./dist/zksync_crypto_bg.wasm');

fs.writeFileSync('./dist/zksync_crypto_wasm.js', `
module.exports = Buffer.from('${buffer.toString('base64')}', 'base64');
`);