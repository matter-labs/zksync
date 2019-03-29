const assert = require('assert')
const RLP = require('../index.js')
const BN = require('bn.js')
const testing = require('ethereumjs-testing')

describe('invalid rlps', function () {
  it('should not crash on an invalid rlp', function () {
    var a = Buffer.from([239, 191, 189, 239, 191, 189, 239, 191, 189, 239, 191, 189, 239, 191, 189, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 239, 191, 189, 29, 239, 191, 189, 77, 239, 191, 189, 239, 191, 189, 239, 191, 189, 93, 122, 239, 191, 189, 239, 191, 189, 239, 191, 189, 103, 239, 191, 189, 239, 191, 189, 239, 191, 189, 26, 239, 191, 189, 18, 69, 27, 239, 191, 189, 239, 191, 189, 116, 19, 239, 191, 189, 239, 191, 189, 66, 239, 191, 189, 64, 212, 147, 71, 239, 191, 189, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 239, 191, 189, 11, 222, 155, 122, 54, 42, 194, 169, 239, 191, 189, 70, 239, 191, 189, 72, 239, 191, 189, 239, 191, 189, 54, 53, 239, 191, 189, 100, 73, 239, 191, 189, 55, 239, 191, 189, 239, 191, 189, 59, 1, 239, 191, 189, 109, 239, 191, 189, 239, 191, 189, 93, 239, 191, 189, 208, 128, 239, 191, 189, 239, 191, 189, 0, 239, 191, 189, 239, 191, 189, 239, 191, 189, 15, 66, 64, 239, 191, 189, 239, 191, 189, 239, 191, 189, 239, 191, 189, 4, 239, 191, 189, 79, 103, 239, 191, 189, 85, 239, 191, 189, 239, 191, 189, 239, 191, 189, 74, 239, 191, 189, 239, 191, 189, 239, 191, 189, 239, 191, 189, 54, 239, 191, 189, 239, 191, 189, 239, 191, 189, 239, 191, 189, 239, 191, 189, 83, 239, 191, 189, 14, 239, 191, 189, 239, 191, 189, 239, 191, 189, 4, 63, 239, 191, 189, 63, 239, 191, 189, 41, 239, 191, 189, 239, 191, 189, 239, 191, 189, 67, 28, 239, 191, 189, 239, 191, 189, 11, 239, 191, 189, 31, 239, 191, 189, 239, 191, 189, 104, 96, 100, 239, 191, 189, 239, 191, 189, 12, 239, 191, 189, 239, 191, 189, 206, 152, 239, 191, 189, 239, 191, 189, 31, 112, 111, 239, 191, 189, 239, 191, 189, 65, 239, 191, 189, 41, 239, 191, 189, 239, 191, 189, 53, 84, 11, 239, 191, 189, 239, 191, 189, 12, 102, 24, 12, 42, 105, 109, 239, 191, 189, 58, 239, 191, 189, 4, 239, 191, 189, 104, 82, 9, 239, 191, 189, 6, 66, 91, 43, 38, 102, 117, 239, 191, 189, 105, 239, 191, 189, 239, 191, 189, 239, 191, 189, 89, 127, 239, 191, 189, 114])
    try {
      RLP.decode(a)
    } catch (e) {
      // FIXME: check for exception name
      assert(true)
    }
  })
})

describe('RLP encoding (string):', function () {
  it('should return itself if single byte and less than 0x7f:', function () {
    var encodedSelf = RLP.encode('a')
    assert.equal(encodedSelf.toString(), 'a')
    assert.equal(RLP.getLength(encodedSelf), 1)
  })

  it('length of string 0-55 should return (0x80+len(data)) plus data', function () {
    var encodedDog = RLP.encode('dog')
    assert.equal(4, encodedDog.length)
    assert.equal(RLP.getLength(encodedDog), 4)
    assert.equal(encodedDog[0], 131)
    assert.equal(encodedDog[1], 100)
    assert.equal(encodedDog[2], 111)
    assert.equal(encodedDog[3], 103)
  })

  it('length of string >55 should return 0xb7+len(len(data)) plus len(data) plus data', function () {
    var encodedLongString = RLP.encode('zoo255zoo255zzzzzzzzzzzzssssssssssssssssssssssssssssssssssssssssssssss')
    assert.equal(72, encodedLongString.length)
    assert.equal(RLP.getLength(encodedLongString), 2)
    assert.equal(encodedLongString[0], 184)
    assert.equal(encodedLongString[1], 70)
    assert.equal(encodedLongString[2], 122)
    assert.equal(encodedLongString[3], 111)
    assert.equal(encodedLongString[12], 53)
  })
})

describe('RLP encoding (list):', function () {
  it('length of list 0-55 should return (0xc0+len(data)) plus data', function () {
    var encodedArrayOfStrings = RLP.encode(['dog', 'god', 'cat'])
    assert.equal(13, encodedArrayOfStrings.length)
    assert.equal(encodedArrayOfStrings[0], 204)
    assert.equal(encodedArrayOfStrings[1], 131)
    assert.equal(encodedArrayOfStrings[11], 97)
    assert.equal(encodedArrayOfStrings[12], 116)
  })

// it('length of list >55 should return 0xf7+len(len(data)) plus len(data) plus data', function () {
//   // need a test case here!
// })
})

describe('RLP encoding (integer):', function () {
  it('length of int = 1, less than 0x7f, similar to string', function () {
    var encodedNumber = RLP.encode(15)
    assert.equal(1, encodedNumber.length)
    assert.equal(encodedNumber[0], 15)
  })

  it('length of int > 55, similar to string', function () {
    var encodedNumber = RLP.encode(1024)
    assert.equal(3, encodedNumber.length)
    assert.equal(encodedNumber[0], 130)
    assert.equal(encodedNumber[1], 4)
    assert.equal(encodedNumber[2], 0)
  })

  it('it should handle zero', function () {
    assert.equal(RLP.encode(0).toString('hex'), '80')
  })
})

describe('RLP decoding (string):', function () {
  it('first byte < 0x7f, return byte itself', function () {
    var decodedStr = RLP.decode(Buffer.from([97]))
    assert.equal(1, decodedStr.length)
    assert.equal(decodedStr.toString(), 'a')
  })

  it('first byte < 0xb7, data is everything except first byte', function () {
    var decodedStr = RLP.decode(Buffer.from([131, 100, 111, 103]))
    assert.equal(3, decodedStr.length)
    assert.equal(decodedStr.toString(), 'dog')
  })

  it('array', function () {
    var decodedBufferArray = RLP.decode(Buffer.from([204, 131, 100, 111, 103, 131, 103, 111, 100, 131, 99, 97, 116]))
    assert.deepEqual(decodedBufferArray, [Buffer.from('dog'), Buffer.from('god'), Buffer.from('cat')])
  })
})

describe('RLP decoding (int):', function () {
  it('first byte < 0x7f, return itself', function () {
    var decodedSmallNum = RLP.decode(Buffer.from([15]))
    assert.equal(1, decodedSmallNum.length)
    assert.equal(decodedSmallNum[0], 15)
  })

  it('first byte < 0xb7, data is everything except first byte', function () {
    var decodedNum = RLP.decode(Buffer.from([130, 4, 0]))
    assert.equal(2, decodedNum.length)
    assert.equal(decodedNum.toString('hex'), '0400')
  })
})

describe('strings over 55 bytes long', function () {
  var testString = 'This function takes in a data, convert it to buffer if not, and a length for recursion'
  testString = Buffer.from(testString)
  var encoded = null
  it('should encode it', function () {
    encoded = RLP.encode(testString)
    assert.equal(encoded[0], 184)
    assert.equal(encoded[1], 86)
  })

  it('should decode', function () {
    var decoded = RLP.decode(encoded)
    assert.equal(decoded.toString(), testString)
  })
})

describe('list over 55 bytes long', function () {
  var testString = ['This', 'function', 'takes', 'in', 'a', 'data', 'convert', 'it', 'to', 'buffer', 'if', 'not', 'and', 'a', 'length', 'for', 'recursion', 'a1', 'a2', 'a3', 'ia4', 'a5', 'a6', 'a7', 'a8', 'ba9']
  var encoded = null

  it('should encode it', function () {
    encoded = RLP.encode(testString)
  })

  it('should decode', function () {
    var decoded = RLP.decode(encoded)
    for (var i = 0; i < decoded.length; i++) {
      decoded[i] = decoded[i].toString()
    }
    assert.deepEqual(decoded, testString)
  })
})

describe('nested lists:', function () {
  var nestedList = [
    [],
    [
      []
    ],
    [
      [],
      [
        []
      ]
    ]
  ]
  var encoded
  it('encode a nested list', function () {
    encoded = RLP.encode(nestedList)
    assert.deepEqual(encoded, Buffer.from([0xc7, 0xc0, 0xc1, 0xc0, 0xc3, 0xc0, 0xc1, 0xc0]))
  })

  it('should decode a nested list', function () {
    var decoded = RLP.decode(encoded)
    assert.deepEqual(nestedList, decoded)
  })
})

describe('null values', function () {
  var nestedList = [null]
  var encoded
  it('encode a null array', function () {
    encoded = RLP.encode(nestedList)
    assert.deepEqual(encoded, Buffer.from([0xc1, 0x80]))
  })

  it('should decode a null value', function () {
    assert.deepEqual(Buffer.from([]), RLP.decode(Buffer.from('80', 'hex')))
  })
})

describe('zero values', function () {
  var encoded
  it('encode a zero', function () {
    encoded = RLP.encode(Buffer.from([0]))
    assert.deepEqual(encoded, Buffer.from([0]))
  })

  it('decode a zero', function () {
    var decode = RLP.decode(Buffer.from([0]))
    assert.deepEqual(decode, Buffer.from([0]))
  })
})

describe('empty values', function () {
  var decoded
  it('decode empty buffer', function () {
    decoded = RLP.decode(Buffer.from([]))
    assert.deepEqual(decoded, Buffer.from([]))
  })
})

describe('bad values', function () {
  it('wrong encoded a zero', function () {
    var val = Buffer.from('f9005f030182520894b94f5374fce5edbc8e2a8697c15331677e6ebf0b0a801ca098ff921201554726367d2be8c804a7ff89ccf285ebc57dff8ae4c44b9c19ac4aa08887321be575c8095f789dd4c743dfe42c1820f9231f98a962b210e3ac2452a3', 'hex')
    var result
    try {
      result = RLP.decode(val)
    } catch (e) {}
    assert.equal(result, undefined)
  })

  it('invalid length', function () {
    var a = Buffer.from('f86081000182520894b94f5374fce5edbc8e2a8697c15331677e6ebf0b0a801ca098ff921201554726367d2be8c804a7ff89ccf285ebc57dff8ae4c44b9c19ac4aa08887321be575c8095f789dd4c743dfe42c1820f9231f98a962b210e3ac2452a3', 'hex')

    var result
    try {
      result = RLP.decode(a)
    } catch (e) {}
    assert.equal(result, undefined)
  })

  it('extra data at end', function () {
    var c = 'f90260f901f9a02a3c692012a15502ba9c39f3aebb36694eed978c74b52e6c0cf210d301dbf325a01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347948888f1f195afa192cfee860698584c030f4c9db1a0ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017a0b6c9fd1447d0b414a1f05957927746f58ef5a2ebde17db631d460eaf6a93b18da0bc37d79753ad738a6dac4921e57392f145d8887476de3f783dfa7edae9283e52b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008302000001832fefd8825208845509814280a00451dd53d9c09f3cfb627b51d9d80632ed801f6330ee584bffc26caac9b9249f88c7bffe5ebd94cc2ff861f85f800a82c35094095e7baea6a6c7c4c2dfeb977efac326af552d870a801ba098c3a099885a281885f487fd37550de16436e8c47874cd213531b10fe751617fa044b6b81011ce57bffcaf610bf728fb8a7237ad261ea2d937423d78eb9e137076c0ef'

    var a = Buffer.from(c, 'hex')

    var result
    try {
      result = RLP.decode(a)
    } catch (e) {}
    assert.equal(result, undefined)
  })

  it('extra data at end', function () {
    var c = 'f9ffffffc260f901f9a02a3c692012a15502ba9c39f3aebb36694eed978c74b52e6c0cf210d301dbf325a01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347948888f1f195afa192cfee860698584c030f4c9db1a0ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017a0b6c9fd1447d0b414a1f05957927746f58ef5a2ebde17db631d460eaf6a93b18da0bc37d79753ad738a6dac4921e57392f145d8887476de3f783dfa7edae9283e52b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008302000001832fefd8825208845509814280a00451dd53d9c09f3cfb627b51d9d80632ed801f6330ee584bffc26caac9b9249f88c7bffe5ebd94cc2ff861f85f800a82c35094095e7baea6a6c7c4c2dfeb977efac326af552d870a801ba098c3a099885a281885f487fd37550de16436e8c47874cd213531b10fe751617fa044b6b81011ce57bffcaf610bf728fb8a7237ad261ea2d937423d78eb9e137076c0'

    var a = Buffer.from(c, 'hex')

    var result
    try {
      result = RLP.decode(a)
    } catch (e) {}
    assert.equal(result, undefined)
  })

  it('list length longer than data', function () {
    var c = 'f9ffffffc260f901f9a02a3c692012a15502ba9c39f3aebb36694eed978c74b52e6c0cf210d301dbf325a01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347948888f1f195afa192cfee860698584c030f4c9db1a0ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017a0b6c9fd1447d0b414a1f05957927746f58ef5a2ebde17db631d460eaf6a93b18da0bc37d79753ad738a6dac4921e57392f145d8887476de3f783dfa7edae9283e52b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008302000001832fefd8825208845509814280a00451dd53d9c09f3cfb627b51d9d80632ed801f6330ee584bffc26caac9b9249f88c7bffe5ebd94cc2ff861f85f800a82c35094095e7baea6a6c7c4c2dfeb977efac326af552d870a801ba098c3a099885a281885f487fd37550de16436e8c47874cd213531b10fe751617fa044b6b81011ce57bffcaf610bf728fb8a7237ad261ea2d937423d78eb9e137076c0'

    var a = Buffer.from(c, 'hex')

    var result
    try {
      result = RLP.decode(a)
    } catch (e) {}
    assert.equal(result, undefined)
  })
})

describe('hex prefix', function () {
  it('should have the same value', function () {
    var a = RLP.encode('0x88f')
    var b = RLP.encode('88f')
    assert.notEqual(a.toString('hex'), b.toString('hex'))
  })
})

describe('offical tests', function () {
  it('pass all tests', function (done) {
    const officalTests = testing.getSingleFile('RLPTests/rlptest.json')

    for (var test in officalTests) {
      var incoming = officalTests[test].in
      // if we are testing a big number
      if (incoming[0] === '#') {
        var bn = new BN(incoming.slice(1))
        incoming = Buffer.from(bn.toArray())
      }

      var encoded = RLP.encode(incoming)
      assert.equal(encoded.toString('hex'), officalTests[test].out.toLowerCase())
    }
    done()
  })
})
