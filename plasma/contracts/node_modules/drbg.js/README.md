# drbg.js

[![NPM Package](https://img.shields.io/npm/v/drbg.js.svg?style=flat-square)](https://www.npmjs.org/package/drbg.js)
[![Build Status](https://img.shields.io/travis/cryptocoinjs/drbg.js.svg?branch=master&style=flat-square)](https://travis-ci.org/cryptocoinjs/drbg.js)
[![Dependency status](https://img.shields.io/david/cryptocoinjs/drbg.js.svg?style=flat-square)](https://david-dm.org/cryptocoinjs/drbg.js#info=dependencies)

[![js-standard-style](https://cdn.rawgit.com/feross/standard/master/badge.svg)](https://github.com/feross/standard)

Deterministic Random Bits Generators

Based on NIST Recommended DRBG from [NIST SP800-90A](https://en.wikipedia.org/wiki/NIST_SP_800-90A) with the following properties:
  * <s>CTR DRBG with DF with AES-128, AES-192, AES-256 cores</s> see [issue #1](https://github.com/cryptocoinjs/drbg.js/issues/1)
  * Hash DRBG with DF with SHA-1, SHA-224, SHA-256, SHA-384, SHA-512 cores
  * HMAC DRBG with SHA-1, SHA-224, SHA-256, SHA-384, SHA-512 cores
  * <s>with</s> and without prediction resistance

## Installation

```shell
npm install drbg.js
```

## Usage

```javascript
var drbgs = require('drbg.js') // import HashDRBG and HmacDRBG
var HashDRBG = drbgs.HashDRBG // or require('drbg.js/hash')
var HmacDRBG = drbgs.HmacDRBG // or require('drbg.js/hmac')

var drbg2 = new HashDRBG('sha256', entropy, nonce, personalization_data)
drbg2.generate(5, additional_data) // <Buffer qq qq qq qq qq>
drbg2.reseed(entropy, personalization_data)
drbg2.generate(5, additional_data) // <Buffer ww ww ww ww ww>

var drbg3 = new HmacDRBG('sha256', entropy, nonce, personalization_data)
drbg3.generate(5, additional_data) // <Buffer ee ee ee ee ee>
drbg3.reseed(entropy, personalization_data)
drbg3.generate(5, additional_data) // <Buffer rr rr rr rr rr>
```

## LICENSE

This library is free and open-source software released under the MIT license.
