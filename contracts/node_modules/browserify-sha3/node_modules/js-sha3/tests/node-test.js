expect = require('expect.js');

// Node.js env
var sha3 = require('../src/sha3.js');
keccak512 = sha3.keccak512;
keccak384 = sha3.keccak384;
keccak256 = sha3.keccak256;
keccak224 = sha3.keccak224;
sha3_512 = sha3.sha3_512;
sha3_384 = sha3.sha3_384;
sha3_256 = sha3.sha3_256;
sha3_224 = sha3.sha3_224;
shake128 = sha3.shake128;
shake256 = sha3.shake256;
cshake128 = sha3.cshake128;
cshake256 = sha3.cshake256;
kmac128 = sha3.kmac128;
kmac256 = sha3.kmac256;
require('./test.js');
require('./test-shake.js');
require('./test-cshake.js');
require('./test-kmac.js');

delete require.cache[require.resolve('../src/sha3.js')];
delete require.cache[require.resolve('./test.js')];
delete require.cache[require.resolve('./test-shake.js')];
delete require.cache[require.resolve('./test-cshake.js')];
delete require.cache[require.resolve('./test-kmac.js')];

// Webpack browser env
JS_SHA3_NO_NODE_JS = true;
window = global;
expect = require('expect.js');
var sha3 = require('../src/sha3.js');
keccak512 = sha3.keccak512;
keccak384 = sha3.keccak384;
keccak256 = sha3.keccak256;
keccak224 = sha3.keccak224;
sha3_512 = sha3.sha3_512;
sha3_384 = sha3.sha3_384;
sha3_256 = sha3.sha3_256;
sha3_224 = sha3.sha3_224;
shake128 = sha3.shake128;
shake256 = sha3.shake256;
cshake128 = sha3.cshake128;
cshake256 = sha3.cshake256;
kmac128 = sha3.kmac128;
kmac256 = sha3.kmac256;
require('./test.js');
require('./test-shake.js');
require('./test-cshake.js');
require('./test-kmac.js');

delete require.cache[require.resolve('../src/sha3.js')];
delete require.cache[require.resolve('./test.js')];
delete require.cache[require.resolve('./test-shake.js')];
delete require.cache[require.resolve('./test-cshake.js')];
delete require.cache[require.resolve('./test-kmac.js')];
sha3_512 = null;
sha3_384 = null;
sha3_256 = null;
sha3_224 = null;
keccak512 = null;
keccak384 = null;
keccak256 = null;
keccak224 = null;
shake128 = null;
shake256 = null;
kmac128 = null;
kmac256 = null;

// browser env
JS_SHA3_NO_NODE_JS = true;
JS_SHA3_NO_COMMON_JS = true;
window = global;
require('../src/sha3.js');
require('./test.js');
require('./test-shake.js');
require('./test-cshake.js');
require('./test-kmac.js');
