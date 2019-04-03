const Ganache = require('ganache-core');
const provider = Ganache.provider();
const Eth = require('ethjs-query');
const EthFilter = require('../index.js');
const assert = require('chai').assert;
const sha3 = require('ethjs-sha3');  // eslint-disable-line
const abi = require('ethjs-abi');    // eslint-disable-line
console.warn = function warn() {}; // eslint-disable-line

describe('EthFilter', () => {
  describe('constructor', () => {
    it('should construct properly', () => {
      const eth = new Eth(provider);
      const filters = new EthFilter(eth);

      assert.equal(typeof filters.Filter, 'function');
      assert.equal(typeof filters.BlockFilter, 'function');
      assert.equal(typeof filters.PendingTransactionFilter, 'function');
    });

    it('should throw under bad construction', () => {
      assert.throws(() => {
        new EthFilter(4354); // eslint-disable-line
      }, Error);
      assert.throws(() => {
        EthFilter({}); // eslint-disable-line
      }, Error);
      assert.throws(() => {
        EthFilter(); // eslint-disable-line
      }, Error);
    });
  });

  describe('Filter', () => {
    it('should construct the Filter object properly', () => {
      const eth = new Eth(provider);
      const filters = new EthFilter(eth);
      const filter = new filters.Filter();

      assert.equal(typeof filter, 'object');
      assert.equal(typeof filter.at, 'function');
      assert.equal(typeof filter.new, 'function');
      assert.equal(typeof filter.watch, 'function');
      assert.equal(typeof filter.uninstall, 'function');
    });

    it('should set filter id properly with the .at method', () => {
      const eth = new Eth(provider);
      const filters = new EthFilter(eth);
      const filter = new filters.Filter();
      filter.at(7);
      assert.equal(filter.filterId, 7);
    });

    it('setup filter with custom delay', () => {
      const eth = new Eth(provider);
      const filters = new EthFilter(eth);
      const filter = new filters.Filter({ delay: 400 });
      filter.at(7);
      assert.equal(filter.options.delay, 400);
    });

    it('should setup a new filter and uninstall with callbacks', (done) => {
      const eth = new Eth(provider);
      const filters = new EthFilter(eth);
      const filter = new filters.Filter();
      filter.new((error, result) => {
        assert.equal(error, null);
        assert.equal(typeof result, 'object');
        assert.equal(filter.filterId.toNumber(10) >= 0, true);

        filter.uninstall((uninstallError, uninstallResult) => {
          assert.equal(uninstallError, null);
          assert.equal(typeof uninstallResult, 'boolean');

          done();
        });
      });
    });

    it('should setup a new filter and uninstall with callbacks and custom object', (done) => {
      const eth = new Eth(provider);
      const filters = new EthFilter(eth);
      const filter = new filters.Filter();
      filter.new({ fromBlock: 0 }, (error, result) => {
        assert.equal(error, null);
        assert.equal(typeof result, 'object');
        assert.equal(filter.filterId.toNumber(10) >= 0, true);

        filter.uninstall((uninstallError, uninstallResult) => {
          assert.equal(uninstallError, null);
          assert.equal(typeof uninstallResult, 'boolean');

          done();
        });
      });
    });

    it('should setup a new filter and handle error', (done) => {
      function FakeProvider() {
        const self = this;
        self.provider = provider;
      }

      FakeProvider.prototype.sendAsync = function sendAsync(payload, callback) {
        const self = this;

        if (payload.method === 'eth_newFilter') {
          const fakeEventLog = {
            id: payload.id,
            jsonrpc: payload.jsonrpc,
            error: 'invalid data!',
            result: [2442384289],
          };

          self.provider.sendAsync(fakeEventLog, callback);
        } else {
          self.provider.sendAsync(payload, callback);
        }
      };

      const eth = new Eth(new FakeProvider());
      const filters = new EthFilter(eth);
      const filter = new filters.Filter();
      filter.new({ fromBlock: 0 }, (error, result) => {
        assert.equal(typeof error, 'object');
        assert.equal(result, null);

        done();
      });
    });

    it('should setup a uninstall filter and handle error', (done) => {
      function FakeProvider() {
        const self = this;
        self.provider = provider;
      }

      FakeProvider.prototype.sendAsync = function sendAsync(payload, callback) {
        const self = this;

        if (payload.method === 'eth_uninstallFilter') {
          const fakeEventLog = {
            id: payload.id,
            jsonrpc: payload.jsonrpc,
            error: 'invalid data!',
            result: [2442384289],
          };

          self.provider.sendAsync(fakeEventLog, callback);
        } else {
          self.provider.sendAsync(payload, callback);
        }
      };

      const eth = new Eth(new FakeProvider());
      const filters = new EthFilter(eth);
      const filter = new filters.Filter();
      filter.new({ fromBlock: 0 }, (error, result) => {
        assert.equal(error, null);
        assert.equal(typeof result, 'object');
        assert.equal(filter.filterId.toNumber(10) >= 0, true);

        filter.uninstall((uninstallError, uninstallResult) => {
          assert.equal(typeof uninstallError, 'object');
          assert.equal(uninstallResult, null);

          done();
        });
      });
    });

    it('should setup a uninstall BlockFilter and handle error', (done) => {
      function FakeProvider() {
        const self = this;
        self.provider = provider;
      }

      FakeProvider.prototype.sendAsync = function sendAsync(payload, callback) {
        const self = this;

        if (payload.method === 'eth_uninstallFilter') {
          const fakeEventLog = {
            id: payload.id,
            jsonrpc: payload.jsonrpc,
            error: 'invalid data!',
            result: [2442384289],
          };

          self.provider.sendAsync(fakeEventLog, callback);
        } else {
          self.provider.sendAsync(payload, callback);
        }
      };

      const eth = new Eth(new FakeProvider());
      const filters = new EthFilter(eth);
      const filter = new filters.BlockFilter();
      filter.new({ fromBlock: 0 }, (error, result) => {
        assert.equal(error, null);
        assert.equal(typeof result, 'object');
        assert.equal(filter.filterId.toNumber(10) >= 0, true);

        filter.uninstall((uninstallError, uninstallResult) => {
          assert.equal(typeof uninstallError, 'object');
          assert.equal(uninstallResult, null);

          done();
        });
      });
    });

    it('should setup a new filter and uninstall with promise and custom object', (done) => {
      const eth = new Eth(provider);
      const filters = new EthFilter(eth);
      const filter = new filters.Filter();
      filter.new({ fromBlock: 0 })
      .catch((error) => {
        assert.equal(error, null);
      })
      .then((result) => {
        assert.equal(typeof result, 'object');
        assert.equal(filter.filterId.toNumber(10) >= 0, true);

        filter.uninstall()
        .catch((uninstallError) => {
          assert.equal(uninstallError, null);
        })
        .then((uninstallResult) => {
          assert.equal(typeof uninstallResult, 'boolean');

          done();
        });
      });
    });

    it('Filter watch should catch thrown decoder error', (done) => {
      function FakeProvider() {
        const self = this;
        self.provider = provider;
      }

      FakeProvider.prototype.sendAsync = function sendAsync(payload, callback) {
        const self = this;

        if (payload.method === 'eth_getFilterChanges') {
          callback(null, { result: [{
            logIndex: '0x0',
            blockNumber: '0x1b4',
            blockHash: '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d54',
            transactionHash: '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dc23f',
            transactionIndex: '0x0',
            address: '0x16c5785ac562ff41e2dcfdf829c5a142f1fccd7d',
            data: '0x0000000000000000000000000000000000000000000000000000000000001194000000000000000000000000ca35b7d915458ef540ade6068dfe2f44e8fa733c',
            topics: ['0x59ebeb90bc63057b6515673c3ecf9438e5058bca0f92585014eced636878c9a5'],
          }] });
        } else {
          self.provider.sendAsync(payload, callback);
        }
      };

      const eth = new Eth(new FakeProvider());
      const filters = new EthFilter(eth);

      eth.accounts((accountsError, accounts) => {
        var count = 0; // eslint-disable-line

        const filter = new filters.Filter({
          decoder: () => { throw new Error('invalid data!!!'); },
        });

        filter.new({ fromBlock: 0, toBlock: 'latest', address: accounts[0] })
        .catch((filterError) => {
          assert.equal(filterError, null);
        })
        .then((filterId) => {
          assert.equal(typeof filterId, 'object');
          filter.watch((watchError, watchResult) => {
            assert.equal(typeof watchError, 'object');
            assert.equal(watchResult, null);

            /*
            filter.uninstall()
            .then((result) => {
              assert.equal(typeof result, 'boolean');
              done();
            })
            .catch(err => assert.isOk(err)); */
          });

          done();
        });
      });
    });

    it('Filter watch and stopWatching should function properly', (done) => {
      const eth = new Eth(provider);
      const filters = new EthFilter(eth);

      eth.accounts((accountsError, accounts) => {
        var count = 0; // eslint-disable-line

        const filter = new filters.Filter();
        filter.new({ fromBlock: 0, toBlock: 'latest', address: accounts[0] })
        .catch((filterError) => {
          assert.equal(filterError, null);
        })
        .then((filterId) => {
          assert.equal(typeof filterId, 'object');

          const watcher = filter.watch((watchError, watchResult) => {
            assert.equal(watchError, null);
            assert.equal(Array.isArray(watchResult), true);
          });

          setTimeout(() => {
            watcher.stopWatching();
            done();
          }, 1400);

          eth.sendTransaction({
            from: accounts[0],
            to: accounts[1],
            value: 3000,
            gas: 3000000,
            data: '0x',
          }, (txError, txResult) => {
            assert.equal(txError, null);
            assert.equal(typeof txResult, 'string');
          });
        });
      });
    });

    it('Filter watch and uninstall should function properly', (done) => {
      const eth = new Eth(provider);
      const filters = new EthFilter(eth);

      eth.accounts((accountsError, accounts) => {
        var count = 0; // eslint-disable-line

        const filter = new filters.Filter();
        filter.new({ fromBlock: 0, toBlock: 'latest', address: accounts[0] })
        .catch((filterError) => {
          assert.equal(filterError, null);
        })
        .then((filterId) => {
          assert.equal(typeof filterId, 'object');

          filter.watch((watchError, watchResult) => {
            assert.equal(watchError, null);
            assert.equal(Array.isArray(watchResult), true);
          });

          setTimeout(() => {
            assert.equal(Object.keys(filter.watchers).length, 1);

            filter.uninstall().then((uninstallResult) => {
              assert.equal(typeof uninstallResult, 'boolean');
              assert.equal(Object.keys(filter.watchers).length, 0);
              done();
            });
          }, 1400);

          eth.sendTransaction({
            from: accounts[0],
            to: accounts[1],
            value: 3000,
            gas: 3000000,
            data: '0x',
          }, (txError, txResult) => {
            assert.equal(txError, null);
            assert.equal(typeof txResult, 'string');
          });
        });
      });
    });

    it('Filter watch and uninstall should function properly with logs', (done) => {
      function FakeProvider() {
        const self = this;
        self.provider = provider;
      }

      FakeProvider.prototype.sendAsync = function sendAsync(payload, callback) {
        const self = this;

        if (payload.method === 'eth_getFilterChanges') {
          callback(null, { result: [{
            logIndex: '0x0',
            blockNumber: '0x1b4',
            blockHash: '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d54',
            transactionHash: '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dc23f',
            transactionIndex: '0x0',
            address: '0x16c5785ac562ff41e2dcfdf829c5a142f1fccd7d',
            data: '0x0000000000000000000000000000000000000000000000000000000000001194000000000000000000000000ca35b7d915458ef540ade6068dfe2f44e8fa733c',
            topics: ['0x59ebeb90bc63057b6515673c3ecf9438e5058bca0f92585014eced636878c9a5'],
          }] });
        } else {
          self.provider.sendAsync(payload, callback);
        }
      };

      const eth = new Eth(new FakeProvider());
      const filters = new EthFilter(eth);

      eth.accounts((accountsError, accounts) => {
        var count = 0; // eslint-disable-line

        const filter = new filters.Filter();
        filter.new({ fromBlock: 0, toBlock: 'latest', address: accounts[0] })
        .catch((filterError) => {
          assert.equal(filterError, null);
        })
        .then((filterId) => {
          assert.equal(typeof filterId, 'object');

          filter.watch((watchError, watchResult) => {
            assert.equal(watchError, null);
            assert.equal(Array.isArray(watchResult), true);
            assert.equal(watchResult.length, 1);
            assert.equal(watchResult[0].logIndex.toNumber(10) >= 0, true);
          });

          setTimeout(() => {
            assert.equal(Object.keys(filter.watchers).length, 1);

            filter.uninstall().then((uninstallResult) => {
              assert.equal(typeof uninstallResult, 'boolean');
              assert.equal(Object.keys(filter.watchers).length, 0);
              done();
            });
          }, 1400);

          eth.sendTransaction({
            from: accounts[0],
            to: accounts[1],
            value: 3000,
            gas: 3000000,
            data: '0x',
          }, (txError, txResult) => {
            assert.equal(txError, null);
            assert.equal(typeof txResult, 'string');
          });
        });
      });
    });
  });
});
