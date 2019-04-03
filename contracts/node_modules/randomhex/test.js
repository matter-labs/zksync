var chai = require('chai');
var assert = chai.assert;
var randomHex = require('./src/index.js');


describe('randomHex', function () {
  it('should generate random bytes with a specific length sync', function () {

    assert.equal(randomHex(0).length, 2 + 0)
    assert.equal(randomHex(3).length, 2 + 6)
    assert.equal(randomHex(30).length, 2 + 60)
    assert.equal(randomHex(300).length, 2 + 600)
    assert.isTrue(/^0x[a-f0-9]+$/.test(randomHex(300)));
  });

  it('should generate random bytes with a specific length async', function (done) {

    randomHex(0, function (err, resp) {
      if (err) throw err

      assert.equal(resp.length, 2 + 0);
      done();
    })
  });
  it('should generate random bytes with a specific length async', function (done) {

    randomHex(3, function (err, resp) {
      if (err) throw err

      assert.equal(resp.length, 2 + 6);
      done();
    })
  }); 
  it('should generate random bytes with a specific length async', function (done) {

    randomHex(30, function (err, resp) {
      if (err) throw err

      assert.equal(resp.length, 2 + 60);
      assert.isTrue(/^0x[a-f0-9]+$/.test(resp));
      done();
    })
  });
  it('should generate random bytes with a specific length async', function (done) {

    randomHex(300, function (err, resp) {
      if (err) throw err

      assert.equal(resp.length, 2 + 600);
      done();
    })
  });


  it('requesting to much throws sync', function () {
    assert.throws(randomHex.bind(null, 65537));
  });

  it('requesting to much throws async', function (done) {
    
    randomHex(65537, function (err, res) {

      assert.isTrue(err instanceof Error);
      done();
    })

  });
});

