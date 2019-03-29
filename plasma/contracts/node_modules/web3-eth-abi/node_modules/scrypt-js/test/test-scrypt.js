var nodeunit = require('nodeunit');

var scrypt = require('../scrypt.js');

var testVectors = require('./test-vectors.json');

function makeTest(options) {
    var password = new Buffer(options.password, 'hex');
    var salt = new Buffer(options.salt, 'hex');
    var N = options.N;
    var p = options.p;
    var r = options.r;
    var dkLen = options.dkLen;

    var derivedKeyHex = options.derivedKey;

    return function (test) {
        scrypt(password, salt, N, r, p, dkLen, function(error, progress, key) {
            if (error) {
                console.log(error);

            } else if (key) {
                key = new Buffer(key);
                test.equal(derivedKeyHex, key.toString('hex'), 'failed to generate correct derived key');
                test.done();
            } else {
            }
        });
    }
}

var Tests = {scrypt: {}};
for (var i = 0; i < testVectors.length; i++) {
    var test = testVectors[i];
    Tests.scrypt['test-' + Object.keys(Tests.scrypt).length] = makeTest(test);
}


nodeunit.reporters.default.run(Tests);
