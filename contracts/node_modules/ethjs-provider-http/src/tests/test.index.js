const HttpProvider = require('../index.js'); // eslint-disable-line
const TestRPC = require('ethereumjs-testrpc'); // eslint-disable-line
const Eth = require('ethjs-query'); // eslint-disable-line
const EthQuery = require('eth-query');
const Web3 = require('web3');
const assert = require('chai').assert; // eslint-disable-line
const SandboxedModule = require('sandboxed-module');
const server = TestRPC.server();
server.listen(5002);

function FakeXHR2() {
  const self = this;
  self.responseText = '{}';
  self.readyState = 4;
  self.onreadystatechange = null;
  self.async = true;
  self.headers = {
    'Content-Type': 'text/plain',
  };
}

FakeXHR2.prototype.open = function(method, host) { // eslint-disable-line
  const self = this;
  assert.equal(method, 'POST');
  assert.notEqual(host, null);
  self.async = true;
};

FakeXHR2.prototype.setRequestHeader = function(name, value) { // eslint-disable-line
  const self = this;
  self.headers[name] = value;
};

FakeXHR2.prototype.send = function (payload) { // eslint-disable-line
  const self = this;
  const payloadParsed = JSON.parse(payload);

  if (payloadParsed.forceTimeout === true) {
    setTimeout(() => {
      self.ontimeout();
    }, 2000);
  } else if (payloadParsed.invalidSend === true) {
    throw new Error('invalid data!!!');
  } else if (payloadParsed.invalidJSON === true) {
    self.responseText = 'dsfsfd{sdf}';
    self.onreadystatechange();
  } else {
    assert.equal(typeof self.onreadystatechange, 'function');
    self.onreadystatechange();
  }
};

SandboxedModule.registerBuiltInSourceTransformer('istanbul');
const FakeHttpProvider = SandboxedModule.require('../index.js', {
  requires: {
    xhr2: FakeXHR2,
  },
  singleOnly: true,
});

describe('HttpProvider', () => {
  describe('constructor', () => {
    it('should throw under invalid conditions', () => {
      assert.throws(() => HttpProvider(''), Error); // eslint-disable-line
      assert.throws(() => new HttpProvider({}, 3932), Error); // eslint-disable-line
    });

    it('should construct normally under valid conditions', () => {
      const provider = new HttpProvider('http://localhost:8545');
      assert.equal(provider.host, 'http://localhost:8545');
      assert.equal(provider.timeout, 0);
    });

    it('should construct normally under valid conditions', () => {
      const provider = new HttpProvider('http://localhost:8545', 10);
      assert.equal(provider.host, 'http://localhost:8545');
      assert.equal(provider.timeout, 10);
    });

    it('should construct normally under valid conditions', () => {
      const provider = new HttpProvider('http://localhost:5002', 10);
      assert.equal(provider.host, 'http://localhost:5002');
      assert.equal(provider.timeout, 10);
    });

    it('should throw error with no new', () => {
      function invalidProvider() {
        HttpProvider('http://localhost:8545', 10); // eslint-disable-line
      }
      assert.throws(invalidProvider, Error);
    });

    it('should throw error with no provider', () => {
      function invalidProvider() {
        new HttpProvider(); // eslint-disable-line
      }
      assert.throws(invalidProvider, Error);
    });
  });

  describe('test against ethjs-query', () => {
    const eth = new Eth(new HttpProvider('http://localhost:5002')); // eslint-disable-line

    it('should get accounts', (done) => {
      eth.accounts((accountsError, accountsResult) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accountsResult, 'object');
        assert.equal(Array.isArray(accountsResult), true);

        done();
      });
    });

    it('should get balances', (done) => {
      eth.accounts((accountsError, accountsResult) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accountsResult, 'object');
        assert.equal(Array.isArray(accountsResult), true);

        eth.getBalance(accountsResult[0], (balanceError, balanceResult) => {
          assert.equal(balanceError, null);
          assert.equal(typeof balanceResult, 'object');
          assert.equal(balanceResult.toNumber(10) > 0, true);

          done();
        });
      });
    });

    it('should get coinbase and balance', (done) => {
      eth.coinbase((accountsError, accountResult) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accountResult, 'string');

        eth.getBalance(accountResult, (balanceError, balanceResult) => {
          assert.equal(balanceError, null);
          assert.equal(typeof balanceResult, 'object');
          assert.equal(balanceResult.toNumber(10) > 0, true);

          done();
        });
      });
    });
  });

  describe('test against eth-query', () => {
    const query = new EthQuery(new HttpProvider('http://localhost:5002')); // eslint-disable-line

    it('should get accounts', (done) => {
      query.accounts((accountsError, accountsResult) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accountsResult, 'object');
        assert.equal(Array.isArray(accountsResult), true);

        done();
      });
    });

    it('should get balances', (done) => {
      query.accounts((accountsError, accountsResult) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accountsResult, 'object');
        assert.equal(Array.isArray(accountsResult), true);

        query.getBalance(accountsResult[0], (balanceError, balanceResult) => {
          assert.equal(balanceError, null);
          assert.equal(typeof balanceResult, 'string');

          done();
        });
      });
    });

    it('should get coinbase and balance', (done) => {
      query.coinbase((accountsError, accountResult) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accountResult, 'string');

        query.getBalance(accountResult, (balanceError, balanceResult) => {
          assert.equal(balanceError, null);
          assert.equal(typeof balanceResult, 'string');

          done();
        });
      });
    });
  });

  describe('test against web3', () => {
    const web3 = new Web3(new HttpProvider('http://localhost:5002')); // eslint-disable-line

    it('should get accounts', (done) => {
      web3.eth.getAccounts((accountsError, accountsResult) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accountsResult, 'object');
        assert.equal(Array.isArray(accountsResult), true);

        done();
      });
    });

    it('should get balances', (done) => {
      web3.eth.getAccounts((accountsError, accountsResult) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accountsResult, 'object');
        assert.equal(Array.isArray(accountsResult), true);

        web3.eth.getBalance(accountsResult[0], (balanceError, balanceResult) => {
          assert.equal(balanceError, null);
          assert.equal(typeof balanceResult, 'object');
          assert.equal(balanceResult.toNumber(10) > 0, true);

          done();
        });
      });
    });

    it('should get coinbase and balance', (done) => {
      web3.eth.getCoinbase((accountsError, accountResult) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accountResult, 'string');

        web3.eth.getBalance(accountResult, (balanceError, balanceResult) => {
          assert.equal(balanceError, null);
          assert.equal(typeof balanceResult, 'object');
          assert.equal(balanceResult.toNumber(10) > 0, true);

          done();
        });
      });
    });

    it('should close the server', () => {
      server.close();
    });
  });

  describe('web3 FakeProvider', () => {
    describe('sendAsync timeout', () => {
      it('should send basic async request and timeout', (done) => {
        const provider = new FakeHttpProvider('http://localhost:5002', 2);

        provider.sendAsync({ forceTimeout: true }, (err, result) => {
          assert.equal(typeof err, 'string');
          assert.equal(typeof result, 'object');
          done();
        });
      });
    });

    describe('invalid payload', () => {
      it('should throw an error as its not proper json', (done) => {
        const provider = new FakeHttpProvider('http://localhost:5002');

        provider.sendAsync('sdfsds{}{df()', (err, result) => {
          assert.equal(typeof err, 'string');
          assert.equal(typeof result, 'object');
          done();
        });
      });

      it('should throw an error as its not proper json', (done) => {
        const provider = new FakeHttpProvider('http://localhost:5002');

        provider.sendAsync({ invalidSend: true }, (err, result) => {
          assert.equal(typeof err, 'string');
          assert.equal(typeof result, 'object');
          done();
        });
      });
    });

    describe('sendAsync timeout', () => {
      it('should send basic async request and timeout', (done) => {
        const provider = new FakeHttpProvider('http://localhost:5002', 2);

        provider.sendAsync({ invalidJSON: true }, (err, result) => {
          assert.equal(typeof err, 'object');
          assert.equal(typeof result, 'string');
          done();
        });
      });
    });

    describe('sendAsync', () => {
      it('should send basic async request', (done) => {
        const provider = new FakeHttpProvider('http://localhost:5002');

        provider.sendAsync({}, (err, result) => {
          assert.equal(typeof result, 'object');
          done();
        });
      });
    });
  });
});
