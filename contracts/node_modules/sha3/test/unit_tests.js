var assert = require('assert');
var util   = require('util');
var SHA3 = require('../').SHA3Hash;

function newBuffer(data, encoding) {
  try {
    return Buffer.from(data, encoding);
  } catch(e) {
    return new Buffer(data, encoding)
  }
}

describe('SHA3', function(){

  describe('constructor', function(){
    it('allows no hash length to be specified', function(){
      assert.doesNotThrow(function(){
        new SHA3();
      });
    });

    it('allows omitting the new keyword', function(){
      assert.doesNotThrow(function(){
        SHA3();
      });
    });

    it('accepts a number to its constructor', function(){
      assert.doesNotThrow(function(){
        new SHA3(224);
        new SHA3(256);
        new SHA3(384);
        new SHA3(512);
      });
    });

    it('throws an error with an integer hashlen of 0', function(){
      assert.throws(function(){
        new SHA3(0);
      }, "TypeError: Unsupported hash length");
    });

    it('throws an error with an integer which is not a supported hash length', function(){
      assert.throws(function(){
        new SHA3(225);
      }, "TypeError: Unsupported hash length");
    });

    it('throws an error with any non-positive integer value', function(){
      assert.throws(function(){
        new SHA3('hi');
      }, "TypeError: Unsupported hash length");
      assert.throws(function(){
        new SHA3(null);
      }, "TypeError: Unsupported hash length");
      assert.throws(function(){
        new SHA3(-1);
      }, "TypeError: Unsupported hash length");
    });
  });

  describe('#update()', function(){
    it('accepts a string as input', function(){
      var sha = new SHA3(224);
      assert.doesNotThrow(function(){
        sha.update('some string value');
      });
    });

    it('accepts a buffer as input', function(){
      var sha = new SHA3(224);

      var buffer = newBuffer('aloha', 'utf8');
      assert.doesNotThrow(function(){
        sha.update(buffer);
      });
    });

    it('does not accept any other types', function(){
      var sha = new SHA3(224);
      [1, 3.14, {}, []].forEach(function(arg){
        assert.throws(function(){
          sha.update(arg);
        }, "TypeError: Not a string or buffer");
      });
    });
  });

  describe('#digest()', function(){
    it('supports hex encoding', function(){
      var result = "0eab42de4c3ceb9235fc91acffe746b29c29a8c366b7c60e4e67c466f36a4304c00" +
                   "fa9caf9d87976ba469bcbe06713b435f091ef2769fb160cdab33d3670680e";
      assert.equal(result, new SHA3().digest('hex'));
    });

    it('supports binary encoding', function(){
      var binary = new SHA3().digest('binary');
      assert.ok(binary);
      assert.ok(binary.length > 0);
    });

    it('defaults to binary encoding', function(){
      var binary = new SHA3().digest();
      assert.ok(binary);
      assert.ok(binary.length > 0);
    });

    it('does not support any other encoding', function(){
      assert.throws(function(){
        new SHA3().digest('buffer');
      }, "TypeError: Unsupported output encoding");
    });

    it('incorporates the updates into the output', function(){
      var sha = new SHA3(224);
      assert.equal('f71837502ba8e10837bdd8d365adb85591895602fc552b48b7390abd', sha.digest('hex'));
      sha.update('some value');
      assert.equal('c6e8a28b9c677c4f5a1098cbc07454cdf7ba7dc4ee600a4655bec0a6', sha.digest('hex'));
    });
  });

  describe('chaining', function(){
    it('can chain', function(){
      assert.equal(
        '76a781712088f94b4f6ca4962f886cac1158bc2f79eabade5ff76d14',
        SHA3(224).update('vlad').digest('hex')
      );
    })
  });
});
