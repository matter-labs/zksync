# secp256k1-node

Version | Mac/Linux | Windows
------- | --------- | -------
[![NPM Package](https://img.shields.io/npm/v/secp256k1.svg?style=flat-square)](https://www.npmjs.org/package/secp256k1) | [![Build Status](https://img.shields.io/travis/cryptocoinjs/secp256k1-node.svg?branch=master&style=flat-square)](https://travis-ci.org/cryptocoinjs/secp256k1-node) | [![AppVeyor](https://img.shields.io/appveyor/ci/fanatid/secp256k1-node.svg?branch=master&style=flat-square)](https://ci.appveyor.com/project/fanatid/secp256k1-node)

[![js-standard-style](https://cdn.rawgit.com/feross/standard/master/badge.svg)](https://github.com/feross/standard)

This module provides native bindings to [bitcoin-core/secp256k1](https://github.com/bitcoin-core/secp256k1). In browser [elliptic](https://github.com/indutny/elliptic) will be used.

This library is experimental, so use at your own risk. Works on node version 4.0.0 or greater.

## Installation

##### from npm

`npm install secp256k1`

##### from git

```
git clone git@github.com:cryptocoinjs/secp256k1-node.git
cd secp256k1-node
git submodule update --init
npm install
```

##### Windows

The easiest way to build the package on windows is to install [windows-build-tools](https://github.com/felixrieseberg/windows-build-tools).

Or install the following software:

  * Git: https://git-scm.com/download/win
  * nvm: https://github.com/coreybutler/nvm-windows
  * Python 2.7: https://www.python.org/downloads/release/python-2712/
  * Visual C++ Build Tools: http://landinghub.visualstudio.com/visual-cpp-build-tools (Custom Install, and select both Windows 8.1 and Windows 10 SDKs)

And run commands:

```
npm config set msvs_version 2015 --global
npm install npm@next -g
```

Based on:

  * https://github.com/nodejs/node-gyp/issues/629#issuecomment-153196245
  * https://github.com/nodejs/node-gyp/issues/972

## Usage

* [API Reference (v3.x)](https://github.com/cryptocoinjs/secp256k1-node/blob/master/API.md)
* [API Reference (v2.x)](https://github.com/cryptocoinjs/secp256k1-node/blob/v2.x/API.md)

```js
const { randomBytes } = require('crypto')
const secp256k1 = require('secp256k1')
// or require('secp256k1/elliptic')
//   if you want to use pure js implementation in node

// generate message to sign
const msg = randomBytes(32)

// generate privKey
let privKey
do {
  privKey = randomBytes(32)
} while (!secp256k1.privateKeyVerify(privKey))

// get the public key in a compressed format
const pubKey = secp256k1.publicKeyCreate(privKey)

// sign the message
const sigObj = secp256k1.sign(msg, privKey)

// verify the signature
console.log(secp256k1.verify(msg, sigObj.signature, pubKey))
// => true
```

\* **.verify return false for high signatures**

## Second pure js implementation

Project has yet one secp256k1 implementation based on [elliptic](http://github.com/indutny/elliptic) and [bn.js](http://github.com/indutny/bn.js). The main purpose of this smaller size, high performance and easy code audit. This implementation is super experimental, use it at your own risk.

## LICENSE

This library is free and open-source software released under the MIT license.
