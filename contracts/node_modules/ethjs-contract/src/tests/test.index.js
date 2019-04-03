require('hard-rejection')();
const EthContract = require('../index.js');
const Eth = require('ethjs-query');
const GanacheCore = require('ganache-core');
const assert = require('chai').assert;
const BN = require('bn.js'); // eslint-disable-line
const asyncWaterfall = require('async/waterfall');
const TestContracts = require('./test-contracts');

describe('EthContract', () => {
  let provider;
  beforeEach(() => {
    provider = GanacheCore.provider();
  });

  describe('should function normally', () => {
    it('should work normally with callbacks', (done) => {
      const eth = new Eth(provider);
      const contract = new EthContract(eth);
      assert.equal(typeof contract, 'function');

      let SimpleStore;
      let newResult;
      let simpleStore;
      let setResult;
      let setNumberValue;

      asyncWaterfall([
        (cb) => {
          eth.accounts(cb);
        },
        (accounts, cb) => {
          assert.equal(Array.isArray(accounts), true);
          SimpleStore = contract(TestContracts.SimpleStore.abi, TestContracts.SimpleStore.bytecode, {
            from: accounts[0],
            gas: 300000,
          });
          SimpleStore.new(cb);
        },
        (_newResult, cb) => {
          newResult = _newResult;
          assert.equal(typeof newResult, 'string');
          cb();
        },
        (cb) => {
          eth.getTransactionReceipt(newResult, cb);
        },
        (receipt, cb) => {
          assert.equal(typeof receipt, 'object');
          assert.equal(typeof receipt.contractAddress, 'string');

          setNumberValue = 4500;
          simpleStore = SimpleStore.at(receipt.contractAddress);

          assert.equal(typeof simpleStore.abi, 'object');
          assert.equal(typeof simpleStore.address, 'string');
          assert.equal(simpleStore.address, receipt.contractAddress);
          assert.equal(typeof simpleStore.set, 'function');
          assert.equal(typeof simpleStore.get, 'function');
          assert.equal(typeof simpleStore.SetComplete, 'function');

          simpleStore.set(setNumberValue, cb);
        },
        (_setResult, cb) => {
          setResult = _setResult;
          assert.equal(typeof setResult, 'string');
          cb();
        },
        (cb) => {
          eth.getTransactionReceipt(setResult, cb);
        },
        (setTxReceipt, cb) => {
          assert.equal(typeof setTxReceipt, 'object');
          simpleStore.get(cb);
        },
        (getResult, cb) => {
          assert.equal(typeof getResult, 'object');
          assert.equal(getResult[0].toNumber(10), setNumberValue);
          cb();
        },
      ], done);
    });

    it('should work normally with promises', async () => {
      const eth = new Eth(provider);
      const { simpleStore } = await deploySimpleStore({ eth });

      const setNumberValue = 4500;
      const setResult = await simpleStore.set(setNumberValue);
      assert.equal(typeof setResult, 'string');

      const setTxReceipt = await eth.getTransactionReceipt(setResult);
      assert.equal(typeof setTxReceipt, 'object');

      const getResult = await simpleStore.get();
      assert.equal(typeof getResult, 'object');
      assert.equal(getResult[0].toNumber(10), setNumberValue);
    });

    it('should use events properly', async () => {
      function FakeProvider() {
        this.provider = provider;
      }

      FakeProvider.prototype.sendAsync = function sendAsync(payload, callback) {
        const self = this;
        const parsedPayload = payload;

        if (parsedPayload.method === 'eth_getFilterChanges') {
          self.provider.sendAsync(payload, () => {
            const fakeEventLog = {
              id: parsedPayload.id,
              jsonrpc: parsedPayload.jsonrpc,
              result: [{
                logIndex: '0x0',
                blockNumber: '0x1b4',
                blockHash: '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d54',
                transactionHash: '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dc23f',
                transactionIndex: '0x0',
                address: '0x16c5785ac562ff41e2dcfdf829c5a142f1fccd7d',
                data: '0x0000000000000000000000000000000000000000000000000000000000001194000000000000000000000000ca35b7d915458ef540ade6068dfe2f44e8fa733c',
                topics: ['0x59ebeb90bc63057b6515673c3ecf9438e5058bca0f92585014eced636878c9a5'],
              }],
            };

            callback(null, fakeEventLog);
          });
        } else {
          self.provider.sendAsync(payload, callback);
        }
      };

      const eth = new Eth(new FakeProvider());
      const { simpleStore } = await deploySimpleStore({ eth });
      const { watchPromise } = await simpleStorePerformSetAndWatchOnce({ simpleStore });

      try {
        await watchPromise;
        // expect to throw
        assert.fail();
      } catch (err) {
        assert.ok(err);
        assert.equal(typeof err, 'object');
      }
    });

    it('should catch watch error under promise', async () => {
      function FakeProvider() {
        this.provider = provider;
      }

      FakeProvider.prototype.sendAsync = function sendAsync(payload, callback) {
        const self = this;
        const parsedPayload = payload;

        if (parsedPayload.method === 'eth_getFilterChanges') {
          self.provider.sendAsync(payload, () => {
            const fakeEventLog = {
              id: parsedPayload.id,
              jsonrpc: parsedPayload.jsonrpc,
              error: 'invalid data',
            };

            callback(null, fakeEventLog);
          });
        } else {
          self.provider.sendAsync(payload, callback);
        }
      };

      const eth = new Eth(new FakeProvider());
      const { simpleStore } = await deploySimpleStore({ eth });
      const { watchPromise } = await simpleStorePerformSetAndWatchOnce({ simpleStore });

      try {
        await watchPromise;
        // expect to throw
        assert.fail();
      } catch (watchErr) {
        assert.ok(watchErr);
        assert.equal(typeof watchErr, 'object');
      }
    });

    it('should catch watch error under promise invalid decode', async () => {
      function FakeProvider() {
        this.provider = provider;
      }

      FakeProvider.prototype.sendAsync = function sendAsync(payload, callback) {
        const self = this;
        const parsedPayload = payload;

        if (parsedPayload.method === 'eth_getFilterChanges') {
          self.provider.sendAsync(payload, () => {
            const fakeEventLog = {
              id: parsedPayload.id,
              jsonrpc: parsedPayload.jsonrpc,
              result: [{
                logIndex: '0x0',
                blockNumber: '0x1b4',
                blockHash: '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d54',
                transactionHash: '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dc23f',
                transactionIndex: '0x0',
                address: '0x16c5785ac562ff41e2dcfdf829c5a142f1fccd7d',
                data: '0xkjdsfkjfskjs',
                topics: ['0x59ebeb90bc63057b6515673c3ecf9438e5058bca0f92585014eced636878c9a5'],
              }],
            };

            callback(null, fakeEventLog);
          });
        } else {
          self.provider.sendAsync(payload, callback);
        }
      };

      const eth = new Eth(new FakeProvider());
      const { simpleStore } = await deploySimpleStore({ eth });
      const { watchPromise } = await simpleStorePerformSetAndWatchOnce({ simpleStore });

      try {
        await watchPromise;
        // expect to throw
        assert.fail();
      } catch (err) {
        assert.ok(err);
        assert.equal(typeof err, 'object');
      }
    });

    it('should catch event watch error', async () => {
      function FakeProvider() {
        this.provider = provider;
      }

      FakeProvider.prototype.sendAsync = function sendAsync(payload, callback) {
        const self = this;
        const parsedPayload = payload;

        if (parsedPayload.method === 'eth_getFilterChanges') {
          self.provider.sendAsync(payload, () => {
            const fakeEventLog = {
              id: parsedPayload.id,
              jsonrpc: parsedPayload.jsonrpc,
              error: 'invalid data',
            };

            callback(null, fakeEventLog);
          });
        } else {
          self.provider.sendAsync(payload, callback);
        }
      };

      const eth = new Eth(new FakeProvider());
      const { simpleStore } = await deploySimpleStore({ eth });
      const { watchPromise } = await simpleStorePerformSetAndWatchOnce({ simpleStore });

      try {
        await watchPromise;
        // expect to throw
        assert.fail();
      } catch (err) {
        assert.ok(err);
        assert.equal(typeof err, 'object');
      }
    });

    it('should handle invalid call data with error', async () => {
      function FakeProvider() {
        this.provider = provider;
      }

      FakeProvider.prototype.sendAsync = function sendAsync(payload, callback) {
        const self = this;
        const parsedPayload = payload;

        if (parsedPayload.method === 'eth_call') {
          self.provider.sendAsync(payload, () => {
            const fakeEventLog = {
              id: parsedPayload.id,
              jsonrpc: parsedPayload.jsonrpc,
              result: '0xkfsdksdfkjfsdjk',
            };

            callback(null, fakeEventLog);
          });
        } else {
          self.provider.sendAsync(payload, callback);
        }
      };

      const eth = new Eth(new FakeProvider());
      const { simpleStore } = await deploySimpleStore({ eth });

      try {
        await simpleStore.get();
        // expect to throw
        assert.fail();
      } catch (err) {
        assert.ok(err);
        assert.equal(typeof err, 'object');
      }
    });

    it('should construct properly with some overriding txObjects', async () => {
      const eth = new Eth(provider);

      const accounts = await eth.accounts();
      const firstFrom = accounts[3];
      const secondFrom = accounts[6];

      const defaultTxObject = { from: firstFrom };
      const { simpleStore, deployTx } = await deploySimpleStore({ eth, defaultTxObject });
      assert.equal(deployTx.from, firstFrom);

      const setOpts = { from: secondFrom };
      const { setTx } = await simpleStorePerformSetAndGet({ eth, simpleStore, setOpts });
      assert.equal(setTx.from, secondFrom);
    });

    it('should construct properly with hexed bytecode', async () => {
      const eth = new Eth(provider);
      const newOpts = {
        data: TestContracts.SimpleStore.bytecode,
      };
      const { simpleStore } = await deploySimpleStore({ eth, newOpts });
      await simpleStorePerformSetAndGet({ eth, simpleStore });
    });

    it('should construct properly with no default tx bytecode', async () => {
      const eth = new Eth(provider);
      const newOpts = {
        data: TestContracts.SimpleStore.bytecode,
      };
      const { simpleStore } = await deploySimpleStore({ eth, newOpts, contractBytecode: null });
      await simpleStorePerformSetAndGet({ eth, simpleStore });
    });

    it('should construct properly with no default tx object when specified in new', async () => {
      const eth = new Eth(provider);
      const accounts = await eth.accounts();
      const newOpts = {
        from: accounts[0],
        gas: 300000,
      };
      const { simpleStore } = await deploySimpleStore({ eth, defaultTxObject: null, newOpts });
      await simpleStorePerformSetAndGet({ eth, simpleStore, setOpts: newOpts });
    });

    it('should construct properly constructor params', async () => {
      const eth = new Eth(provider);
      await deployAndTestComplexStore({ eth });
    });

    it('should construct properly constructor params and overriding tx object', async () => {
      const eth = new Eth(provider);
      const accounts = await eth.accounts();
      const newTxParams = { from: accounts[3] };
      const { deployTx } = await deployAndTestComplexStore({ eth, newTxParams });
      assert.equal(deployTx.from, accounts[3]);
    });

    it('should handle multi-type set and multi-type return', async () => {
      const eth = new Eth(provider);
      const contract = new EthContract(eth);
      const accounts = await eth.accounts();

      const initalValue = 730483222;
      const initalAddressArray = [accounts[3], accounts[2], accounts[1]];
      const SimpleStore = contract(TestContracts.ExtraComplexStore.abi, TestContracts.ExtraComplexStore.bytecode, {
        from: accounts[0],
        gas: 3000000,
      });
      const deployTxHash = await SimpleStore.new(initalValue, initalAddressArray, { from: accounts[3] });
      assert.equal(typeof deployTxHash, 'string');

      const setTxReceipt = await eth.getTransactionReceipt(deployTxHash);
      const extraComplexStore = SimpleStore.at(setTxReceipt.contractAddress);

      const args = [
        // int _val1
        453,
        // uint _val2
        new BN('289234972'),
        // address[] _val3
        [accounts[4], accounts[2]],
        // string _val4
        'some great string',
        // uint8 _val5
        55,
        // bytes32 _val6
        '0x47173285a8d7341e5e972fc677286384f802f8ef42a5ec5f03bbfa254cb01fad',
        // bytes _val7
        '0x47173285a8d73bbfa254cb01fad3',
        // bytes8 _val8
        '0x47173285a8d73b3d',
        // int8 _val9
        2,
        // int16 _val10
        12,
        // uint256[] _val11
        [12342, 923849, new BN('249829233')],
      ];
      const multiTypeSetTxHash = await extraComplexStore.multiTypeSet(...args);
      const multiSetReceipt = await eth.getTransactionReceipt(multiTypeSetTxHash);
      assert.equal(typeof multiSetReceipt, 'object');

      const multiReturn = await extraComplexStore.multiTypeReturn();
      assert.equal(typeof multiReturn, 'object');

      assert.equal(multiReturn[0].toNumber(10), args[0]);
      assert.equal(multiReturn[1].toNumber(10), args[1].toNumber(10));
      assert.equal(multiReturn[2][0], args[2][0]);
      assert.equal(multiReturn[2][1], args[2][1]);
      assert.equal(multiReturn[3], args[3]);
      assert.equal(multiReturn[4].toNumber(10), args[4]);
      assert.equal(multiReturn[5], args[5]);
      assert.equal(multiReturn[6], args[6]);
      assert.equal(multiReturn[7], args[7]);
      assert.equal(multiReturn[8].toNumber(10), args[8]);
      assert.equal(multiReturn[9].toNumber(10), args[9]);
      assert.equal(multiReturn[10][0].toNumber(10), args[10][0]);
      assert.equal(multiReturn[10][1].toNumber(10), args[10][1]);
      assert.equal(multiReturn[10][2].toNumber(10), args[10][2].toNumber(10));
    });
  });
});

async function deploySimpleStore({ eth, defaultTxObject, newOpts = {}, contractBytecode }) {
  const contract = new EthContract(eth);
  assert.equal(typeof contract, 'function');

  const accounts = await eth.accounts();
  assert.equal(Array.isArray(accounts), true);

  // set `defaultTxObject` option to null to omit
  const finalDefaultTxObject = (defaultTxObject === null) ? undefined : Object.assign({
    from: accounts[0],
    gas: 300000,
  }, defaultTxObject);
  // set `contractBytecode` option to null to omit
  const finalContractByteCode = (contractBytecode === null) ? undefined : (contractBytecode || TestContracts.SimpleStore.bytecode);
  const SimpleStore = contract(TestContracts.SimpleStore.abi, finalContractByteCode, finalDefaultTxObject);

  const deployTxHash = await SimpleStore.new(newOpts);
  assert.ok(deployTxHash);
  assert.equal(typeof deployTxHash, 'string');

  const deployTx = await eth.getTransactionByHash(deployTxHash);
  assert.ok(deployTx);
  assert.equal(typeof deployTx, 'object');

  const deployTxRx = await eth.getTransactionReceipt(deployTxHash);
  assert.equal(typeof deployTxRx, 'object');
  assert.equal(typeof deployTxRx.contractAddress, 'string');

  const simpleStore = SimpleStore.at(deployTxRx.contractAddress);
  assert.equal(typeof simpleStore.abi, 'object');
  assert.equal(typeof simpleStore.address, 'string');
  assert.equal(simpleStore.address, deployTxRx.contractAddress);
  assert.equal(typeof simpleStore.set, 'function');
  assert.equal(typeof simpleStore.get, 'function');
  assert.equal(typeof simpleStore.SetComplete, 'function');

  return { simpleStore, deployTx, deployTxRx };
}

async function watchEventOnce(contractEvent) {
  return await new Promise((resolve, reject) => {
    contractEvent.watch((watchErr) => {
      if (watchErr) {
        return reject(watchErr);
      }

      return contractEvent.uninstall((stopWatchingError) => {
        if (stopWatchingError) {
          return reject(stopWatchingError);
        }
        return resolve();
      });
    });
  });
}

async function simpleStorePerformSetAndWatchOnce({ simpleStore }) {
  const setCompleteEvent = simpleStore.SetComplete(); // eslint-disable-line
  const setCompleteFilterId = await setCompleteEvent.new({ fromBlock: 'earliest', toBlock: 'latest' });
  assert.ok(setCompleteFilterId);
  assert.equal(typeof setCompleteFilterId, 'object');
  assert.equal(setCompleteFilterId.toString(10) > 0, true);

  const watchPromise = watchEventOnce(setCompleteEvent);
  const setTxHash = await simpleStore.set(1337);
  assert.equal(typeof setTxHash, 'string');

  return { watchPromise };
}

async function simpleStorePerformSetAndGet({ eth, simpleStore, setNumberValue = 42, setOpts = {} }) {
  const setTxHash = await simpleStore.set(setNumberValue, setOpts);
  assert.equal(typeof setTxHash, 'string');

  const setTx = await eth.getTransactionByHash(setTxHash);
  assert.ok(setTx);
  assert.equal(typeof setTx, 'object');
  const setTxReceipt = await eth.getTransactionReceipt(setTxHash);
  assert.equal(typeof setTxReceipt, 'object');

  const getResult = await simpleStore.get();
  assert.equal(typeof getResult, 'object');
  assert.equal(getResult[0].toNumber(10), setNumberValue);

  return { setTx, setTxReceipt };
}

async function deployAndTestComplexStore({ eth, newTxParams }) {
  const accounts = await eth.accounts();
  const contract = new EthContract(eth);

  const defaultTxObject = { from: accounts[0], gas: 300000 };
  const ComplexStore = contract(TestContracts.ComplexStore.abi, TestContracts.ComplexStore.bytecode, defaultTxObject);

  const initialValue = 730483222;
  const initialAddressArray = [accounts[3], accounts[2], accounts[1]];

  let deployTxHash;
  if (newTxParams) {
    deployTxHash = await ComplexStore.new(initialValue, initialAddressArray, newTxParams);
  } else {
    deployTxHash = await ComplexStore.new(initialValue, initialAddressArray);
  }

  const deployTx = await eth.getTransactionByHash(deployTxHash);
  const deployTxRx = await eth.getTransactionReceipt(deployTxHash);
  const complexStore = ComplexStore.at(deployTxRx.contractAddress);

  const addressResult0 = await complexStore.addresses(0);
  assert.equal(addressResult0[0], initialAddressArray[0]);
  const addressResult1 = await complexStore.addresses(1);
  assert.equal(addressResult1[0], initialAddressArray[1]);
  const addressResult2 = await complexStore.addresses(2);
  assert.equal(addressResult2[0], initialAddressArray[2]);
  const someValue = await complexStore.someVal();
  assert.equal(someValue[0].toNumber(10), initialValue);

  return { deployTx };
}
