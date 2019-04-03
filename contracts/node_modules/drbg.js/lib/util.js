'use strict'
exports.b512zero = new Buffer(256)
;(function () {
  for (var i = 0; i < exports.b512zero.length; ++i) exports.b512zero[i] = 0
})()

exports.bKsource = new Buffer(32)
;(function () {
  for (var i = 0; i < exports.bKsource.length; ++i) exports.bKsource[i] = i
})()

exports.bsum = function (buffers) {
  var dst = new Buffer(buffers[0])

  for (var i = 1; i < buffers.length; ++i) {
    for (var j = buffers[i].length - 1, dj = dst.length - 1, carry = 0; j >= 0; --j, --dj) {
      carry += buffers[i][j] + dst[dj]
      dst[dj] = carry & 0xff
      carry >>>= 8
    }

    for (; carry > 0 && dj >= 0; --dj) {
      carry += dst[dj]
      dst[dj] = carry & 0xff
      carry >>>= 8
    }
  }

  return dst
}

exports.bxor = function (a, b) {
  var r = new Buffer(a.length)
  for (var i = 0; i < r.length; ++i) r[i] = a[i] ^ b[i]
  return r
}
