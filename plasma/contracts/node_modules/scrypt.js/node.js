var scrypt = require('scrypt')

function hash(key, salt, n, r, p, dklen, progressCb) {
  return scrypt.hashSync(key, { N: n, r: r, p: p }, dklen, salt)
}

module.exports = hash
