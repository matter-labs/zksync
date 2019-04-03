const assert = require('chai').assert;
const TestRPC = require('ethereumjs-testrpc');
const provider = TestRPC.provider();
const Eth = require('../index.js');

describe('getTransactionSuccess.js', () => {
  it('should get tx receipt properly', (done) => {
    const eth = new Eth(provider);

    eth.accounts((accErr, accounts) => {
      assert.isNotOk(accErr);

      const defaultTxObject = {
        from: accounts[0],
        to: accounts[1],
        value: (new Eth.BN('4500')),
        data: '0x',
        gas: 300000,
      };

      eth.sendTransaction(defaultTxObject, (txErr, txHash) => {
        assert.isNotOk(txErr);

        eth.getTransactionSuccess(txHash, (succErr, successResult) => {
          assert.isNotOk(succErr);
          assert.isOk(successResult);

          done();
        });
      });
    });
  });

  it('should trigger errors', (done) => {
    const eth = new Eth(provider);

    eth.getTransactionSuccess(33, (succErr) => {
      assert.isOk(succErr);

      done();
    });
  });

  it('should timeout', (done) => {
    const eth = new Eth(provider, { timeout: 1000, interval: 100 });

    eth.getTransactionSuccess('0xec66b273967d58c9611ae8dace378d550ccbd453e9815c78f8d1ffe5bb2aff1c', (succErr) => {
      assert.isOk(succErr);

      done();
    });
  });
});
