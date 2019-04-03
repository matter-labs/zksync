const tape = require('tape')
const Keccak = require('../index')
const KeccakJS = require('../browser')

tape('Auto-detected', function (t) {
  t.test('basic', function (st) {
    var hash = new Keccak()
    hash.update('hello')
    hash.update(Buffer.from('42004200', 'hex'))
    st.equal(hash.digest('hex'), '1f900dfea0147b249861792bcf838a42c0bb276a395a64d90ee08491e5dbce09273846902b57b739b40b9983d21de29678df9e8585f56f532088c80ed41d6354')
    st.end()
  })
})

tape('Javascript', function (t) {
  t.test('basic', function (st) {
    var hash = new KeccakJS()
    hash.update('hello')
    hash.update(Buffer.from('42004200', 'hex'))
    st.equal(hash.digest('hex'), '1f900dfea0147b249861792bcf838a42c0bb276a395a64d90ee08491e5dbce09273846902b57b739b40b9983d21de29678df9e8585f56f532088c80ed41d6354')
    st.end()
  })
})
