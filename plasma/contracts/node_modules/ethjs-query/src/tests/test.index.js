require('hard-rejection')();
const Eth = require('../index.js');
const Eth2 = require('../index.js');
const assert = require('chai').assert;
const util = require('ethjs-util');
const GanacheCore = require('ganache-core');
const BigNumber = require('bn.js');
const abi = require('ethjs-abi');

describe('ethjs-query', () => {
  let provider;

  beforeEach(() => {
    provider = GanacheCore.provider();
  });

  describe('construction', () => {
    it('should construct normally', () => {
      const eth = new Eth(provider);

      assert.equal(typeof eth, 'object');
      assert.equal(typeof eth.accounts, 'function');
      assert.equal(typeof eth.getBalance, 'function');
      assert.equal(typeof eth.sendTransaction, 'function');
      assert.equal(typeof eth.sendRawTransaction, 'function');
      assert.equal(typeof eth.personal_sign, 'function');
      assert.equal(typeof eth.personal_ecRecover, 'function');
    });

    it('should construct normally with non Eth name', () => {
      const eth = new Eth2(provider);

      assert.equal(typeof eth, 'object');
      assert.equal(typeof eth.accounts, 'function');
      assert.equal(typeof eth.getBalance, 'function');
      assert.equal(typeof eth.sendTransaction, 'function');
      assert.equal(typeof eth.sendRawTransaction, 'function');
    });

    it('should fail when provider is not valid', (done) => {
      try {
        const eth = new Eth(''); // eslint-disable-line
      } catch (error) {
        assert.equal(typeof error, 'object');
        done();
      }
    });

    it('should fail when provider is not valid', (done) => {
      try {
        const eth = new Eth(342323); // eslint-disable-line
      } catch (error) {
        assert.equal(typeof error, 'object');
        done();
      }
    });

    it('debugger should function', (done) => {
      const eth = new Eth(provider, { debug: true, logger: { log: (message) => {
        assert.equal(typeof message, 'string');
      }}}); // eslint-disable-line

      eth.accounts((err, result) => {
        assert.equal(err, null);
        assert.equal(Array.isArray(result), true);
        done();
      });
    });

    it('should fail with response error payload', (done) => {
      const eth = new Eth({
        sendAsync: (opts, cb) => {
          cb(false, { error: 'bad data..' });
        },
      }); // eslint-disable-line

      eth.accounts((err, result) => {
        assert.equal(typeof err, 'object');
        assert.equal(result, null);
        done();
      });
    });

    it('should handle empty getTransactionReceipt', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.getTransactionReceipt('0x7f9de10bdd8686734c1b2dd2b7e53ea3e1ffe7fd4698a3a521ec8e09570ca121', (err, result) => {
        assert.equal(typeof err, 'object');
        assert.equal(result, null);
        done();
      });
    });

    it('should fail with invalid payload response (formatting error)', (done) => {
      const eth = new Eth({
        sendAsync: (opts, cb) => {
          cb(false, { result: [38274978, 983428943] });
        },
      }); // eslint-disable-line

      eth.accounts((err, result) => {
        assert.equal(typeof err, 'object');
        assert.equal(result, null);
        done();
      });
    });

    it('should fail with invalid method input (formatting error)', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.getBalance(234842387, (err, result) => {
        assert.equal(typeof err, 'object');
        assert.equal(result, null);
        done();
      });
    });

    it('should fail when no new flag is present', (done) => {
      try {
        const eth = Eth2(provider); // eslint-disable-line
      } catch (error) {
        assert.equal(typeof error, 'object');
        done();
      }
    });

    it('should fail nicely when too little params on getBalance', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.getBalance((err, result) => {
        assert.equal(typeof err, 'object');
        assert.equal(result, null);

        done();
      });
    });

    it('should fail nicely when too many paramsEncoded on getBalance', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.getBalance('fsdfsd', 'sdffsd', 'dsfdfssf', (error, result) => {
        assert.equal(typeof error, 'object');
        assert.equal(result, null);

        done();
      });
    });

    it('should check if the rpc is eth_syncing', (done) => {
      const eth = new Eth(provider);

      eth.syncing((err, result) => {
        assert.equal(err, null);
        assert.equal(typeof result, 'boolean');

        done();
      });
    });

    it('should function while eth_coinbase', (done) => {
      const eth = new Eth(provider);

      eth.coinbase((err, result) => {
        assert.equal(err, null);
        assert.equal(typeof result, 'string');
        assert.equal(util.getBinarySize(result), 42);

        done();
      });
    });

    it('should function while eth_coinbase using promise', (done) => {
      const eth = new Eth(provider);

      eth.coinbase()
      .then((result) => {
        assert.equal(typeof result, 'string');
        assert.equal(util.getBinarySize(result), 42);

        done();
      })
      .catch((err) => {
        assert.equal(err, null);
      });
    });

    it('should get acconts with promise', (done) => {
      const eth = new Eth(provider);

      eth.accounts()
      .then((result) => {
        assert.equal(typeof result, 'object');
        assert.equal(result.length > 0, true);

        done();
      })
      .catch((err) => {
        assert.equal(err, null);
      });
    });

    it('should reject bad getBalance call with an error', (done) => {
      const eth = new Eth(provider);

      eth.accounts((accountsError, accounts) => {
        eth.sendTransaction({
          from: accounts[0],
          to: accounts[1],
          gas: 10,
          value: 100000,
          data: '0x',
        }).catch((err) => {
          assert.equal(typeof err, 'object');
          done();
        });
      });
    });

    it('should function while eth_getBalance using promise', (done) => {
      const eth = new Eth(provider);

      eth.coinbase()
      .then((result) => {
        assert.equal(typeof result, 'string');
        assert.equal(util.getBinarySize(result), 42);

        eth.getBalance(result)
        .then((balance) => {
          assert.equal(typeof balance, 'object');

          done();
        })
        .catch((err) => {
          assert.equal(err, null);
        });
      })
      .catch((err) => {
        assert.equal(err, null);
      });
    });

    it('should function while eth_getBalance, optional and non optional latest', (done) => {
      const eth = new Eth(provider);

      eth.coinbase((err, coinbase) => {
        assert.equal(err, null);
        assert.equal(typeof coinbase, 'string');
        assert.equal(util.getBinarySize(coinbase), 42);

        eth.getBalance(coinbase, (balanceError, balance) => {
          assert.equal(balanceError, null);
          assert.equal(typeof balance, 'object');

          eth.getBalance(coinbase, 'latest', (balanceLatestError, balanceLatest) => {
            assert.equal(balanceLatestError, null);
            assert.equal(typeof balanceLatest, 'object');
            assert.equal(balance.toString(10), balanceLatest.toString(10));

            done();
          });
        });
      });
    });

    it('should function while get_accounts', (done) => {
      const eth = new Eth(provider);

      eth.accounts((err, result) => {
        assert.equal(err, null);
        assert.equal(typeof result, 'object');
        assert.equal(Array.isArray(result), true);
        assert.equal(result.length > 0, true);
        assert.equal(typeof result[0], 'string');
        assert.equal(util.getBinarySize(result[0]), 42);

        done();
      });
    });

    it('should function while eth_blockNumber', (done) => {
      const eth = new Eth(provider);

      eth.blockNumber((err, result) => {
        assert.equal(err, null);
        assert.equal(typeof result, 'object');
        assert.equal(result.toNumber() >= 0, true);
        done();
      });
    });

    it('should function while eth_estimateGas', (done) => {
      const eth = new Eth(provider);
      eth.accounts((accountsError, accounts) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accounts, 'object');

        const testTransactionObject = {
          from: accounts[0],
          to: accounts[4],
          gas: new BigNumber(23472),
          gasPrice: '92384242',
          data: '0x',
        };

        eth.estimateGas(testTransactionObject, (err, result) => {
          assert.equal(err, null);
          assert.equal(typeof result, 'object');
          assert.equal(typeof result.toString(10), 'string');
          assert.equal(result.toNumber(10) > 0, true);
          done();
        });
      });
    });

    it('should function while eth_gasPrice', (done) => {
      const eth = new Eth(provider);

      eth.gasPrice((err, result) => {
        assert.equal(err, null);
        assert.equal(typeof result, 'object');
        assert.equal(result.toNumber() > 0, true);
        done();
      });
    });

    it('should function while eth_getBalance', (done) => {
      const eth = new Eth(provider);

      eth.accounts((accountsError, accounts) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accounts, 'object');

        eth.getBalance(accounts[0], (err, result) => {
          assert.equal(err, null);
          assert.equal(typeof result, 'object');
          assert.equal(result.gt(0), true);

          eth.getBalance(accounts[0], 'latest', (err2, result2) => {
            assert.equal(err2, null);
            assert.equal(typeof result2, 'object');
            assert.equal(result2.gt(0), true);
            done();
          });
        });
      });
    });

    it('should function while eth_getBlockByNumber', (done) => { // eslint-disable-line
      const eth = new Eth(provider);

      eth.getBlockByNumber(0, true, (blockError, result) => {
        assert.equal(blockError, null);
        assert.equal(typeof result, 'object');
        assert.equal(util.getBinarySize(result.hash), 66);
        assert.equal(util.getBinarySize(result.sha3Uncles), 66);
        assert.equal(util.getBinarySize(result.parentHash), 66);
        assert.equal(result.size.toNumber(10) > 0, true);
        assert.equal(result.gasLimit.toNumber(10) > 0, true);
        assert.equal(result.timestamp.toNumber(10) > 0, true);
        done();
      });
    });

    it('should function while eth_getBlockByHash', (done) => {
      const eth = new Eth(provider);

      eth.getBlockByNumber(0, true, (blockError, block) => {
        assert.equal(blockError, null);
        assert.equal(typeof block, 'object');

        eth.getBlockByHash(block.hash, true, (error, result) => {
          assert.equal(error, null);
          assert.equal(typeof result, 'object');
          assert.equal(util.getBinarySize(result.hash), 66);
          assert.equal(util.getBinarySize(result.sha3Uncles), 66);
          assert.equal(util.getBinarySize(result.parentHash), 66);
          assert.equal(result.size.toNumber(10) > 0, true);
          assert.equal(result.gasLimit.toNumber(10) > 0, true);
          assert.equal(result.timestamp.toNumber(10) > 0, true);
          done();
        });
      });
    });

    it('should function while eth_getCode', (done) => {
      const eth = new Eth(provider); // eslint-disable-line
      done();
    });

    it('should function while eth_getCompilers', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.getCompilers((error, result) => {
        assert.equal(error, null);
        assert.equal(typeof result, 'object');
        assert.equal(Array.isArray(result), true);
        assert.equal(typeof result[0], 'string');

        done();
      });
    });

    it('should function while eth_hashrate', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.hashrate((error, result) => {
        assert.equal(error, null);
        assert.equal(typeof result, 'object');
        assert.equal(result.toNumber(10) >= 0, true);

        done();
      });
    });

    it('should function while eth_mining', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.mining((error, result) => {
        assert.equal(error, null);
        assert.equal(typeof result, 'boolean');

        done();
      });
    });

    it('should function while eth_getTransactionCount', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.accounts((accountsError, accounts) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accounts, 'object');

        eth.getTransactionCount(accounts[0], (error, result) => {
          assert.equal(error, null);
          assert.equal(typeof result, 'object');
          assert.equal(result.toNumber(10) >= 0, true);

          done();
        });
      });
    });

    it('should function while eth_getTransactionByBlockHashAndIndex', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.accounts((accountsError, accounts) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accounts, 'object');

        const testTransaction = {
          from: accounts[0],
          to: accounts[2],
          gas: 3000000,
          data: '0x',
        };

        eth.sendTransaction(testTransaction, (error, result) => {
          assert.equal(error, null);
          assert.equal(typeof result, 'string');
          assert.equal(util.getBinarySize(result), 66);

          eth.getTransactionReceipt(result, (receiptError, receipt) => {
            assert.equal(receiptError, null);
            assert.equal(typeof receipt, 'object');

            eth.getTransactionByBlockHashAndIndex(receipt.blockHash, 0, (blockError, block) => {
              assert.equal(blockError, null);
              assert.equal(typeof block, 'object');
              assert.equal(util.getBinarySize(block.blockHash), 66);
              assert.equal(block.gas.toNumber(10) >= 0, true);
              assert.equal(block.gasPrice.toNumber(10) >= 0, true);
              assert.equal(block.transactionIndex.toNumber(10) >= 0, true);
              assert.equal(block.blockNumber.toNumber(10) >= 0, true);

              done();
            });
          });
        });
      });
    });

    it('should function while eth_getTransactionByBlockNumberAndIndex', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.accounts((accountsError, accounts) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accounts, 'object');

        const testTransaction = {
          from: accounts[0],
          to: accounts[2],
          gas: 3000000,
          data: '0x',
        };

        eth.sendTransaction(testTransaction, (error, result) => {
          assert.equal(error, null);
          assert.equal(typeof result, 'string');
          assert.equal(util.getBinarySize(result), 66);

          eth.getTransactionReceipt(result, (receiptError, receipt) => {
            assert.equal(receiptError, null);
            assert.equal(typeof receipt, 'object');

            eth.getTransactionByBlockNumberAndIndex(1, 0, (blockError, block) => {
              assert.equal(blockError, null);
              assert.equal(typeof block, 'object');
              assert.equal(util.getBinarySize(block.blockHash), 66);
              assert.equal(block.gas.toNumber(10) >= 0, true);
              assert.equal(block.gasPrice.toNumber(10) >= 0, true);
              assert.equal(block.transactionIndex.toNumber(10) >= 0, true);
              assert.equal(block.blockNumber.toNumber(10) >= 0, true);

              done();
            });
          });
        });
      });
    });

    it('should function while eth_sendTransaction', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.accounts((accountsError, accounts) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accounts, 'object');

        const testTransaction = {
          from: accounts[0],
          to: accounts[2],
          gas: 3000000,
          data: '0x',
        };

        eth.sendTransaction(testTransaction, (error, result) => {
          assert.equal(error, null);
          assert.equal(typeof result, 'string');
          assert.equal(util.getBinarySize(result), 66);

          done();
        });
      });
    });

    it('should function while eth_sendTransaction with contract', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.accounts((accountsError, accounts) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accounts, 'object');

        const testTransaction = {
          from: accounts[0],
          gas: '3000000',
          data: '606060405234610000575b61016a806100186000396000f360606040526000357c010000000000000000000000000000000000000000000000000000000090048063119c56bd1461004e57806360fe47b11461008e5780636d4ce63c146100c1575b610000565b346100005761005b6100e4565b604051808381526020018273ffffffffffffffffffffffffffffffffffffffff1681526020019250505060405180910390f35b34610000576100a960048080359060200190919050506100f5565b60405180821515815260200191505060405180910390f35b34610000576100ce61015f565b6040518082815260200191505060405180910390f35b60006000610d7d91503390505b9091565b6000816000819055507f10e8e9bc5a1bde3dd6bb7245b52503fcb9d9b1d7c7b26743f82c51cc7cce917d60005433604051808381526020018273ffffffffffffffffffffffffffffffffffffffff1681526020019250505060405180910390a1600190505b919050565b600060005490505b9056',
        };

        eth.sendTransaction(testTransaction, (error, result) => {
          assert.equal(error, null);
          assert.equal(typeof result, 'string');
          assert.equal(util.getBinarySize(result), 66);

          done();
        });
      });
    });

    it('should function while eth_sign', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.accounts((accountsError, accounts) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accounts, 'object');

        const testTxData = '0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470';

        eth.sign(accounts[0], testTxData, (error, result) => {
          assert.equal(error, null);
          assert.equal(typeof result, 'string');
          assert.equal(util.getBinarySize(result) > 0, true);

          done();
        });
      });
    });

    it('should function while eth_getTransactionReceipt', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.accounts((accountsError, accounts) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accounts, 'object');

        const testTransaction = {
          from: accounts[0],
          to: accounts[2],
          gas: 3000000,
          data: '0x',
        };

        eth.sendTransaction(testTransaction, (error, result) => {
          assert.equal(error, null);
          assert.equal(typeof result, 'string');
          assert.equal(util.getBinarySize(result), 66);

          setTimeout(() => {
            eth.getTransactionReceipt(result, (receiptError, receipt) => {
              assert.equal(receiptError, null);
              assert.equal(typeof receipt, 'object');

              assert.equal(util.getBinarySize(receipt.transactionHash), 66);
              assert.equal(receipt.transactionIndex.toNumber(10) >= 0, true);
              assert.equal(receipt.blockNumber.toNumber(10) >= 0, true);
              assert.equal(receipt.cumulativeGasUsed.toNumber(10) >= 0, true);
              assert.equal(receipt.gasUsed.toNumber(10) >= 0, true);
              assert.equal(Array.isArray(receipt.logs), true);

              done();
            });
          }, 340);
        });
      });
    });

    it('should function while deploy, use contract via eth_call, eth_getCode', (done) => {
      const eth = new Eth(provider); // eslint-disable-line

      eth.accounts((accountsError, accounts) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accounts, 'object');

        const testContractTransaction = {
          from: accounts[0],
          gas: 3000000,
          data: '606060405234610000575b61016a806100186000396000f360606040526000357c010000000000000000000000000000000000000000000000000000000090048063119c56bd1461004e57806360fe47b11461008e5780636d4ce63c146100c1575b610000565b346100005761005b6100e4565b604051808381526020018273ffffffffffffffffffffffffffffffffffffffff1681526020019250505060405180910390f35b34610000576100a960048080359060200190919050506100f5565b60405180821515815260200191505060405180910390f35b34610000576100ce61015f565b6040518082815260200191505060405180910390f35b60006000610d7d91503390505b9091565b6000816000819055507f10e8e9bc5a1bde3dd6bb7245b52503fcb9d9b1d7c7b26743f82c51cc7cce917d60005433604051808381526020018273ffffffffffffffffffffffffffffffffffffffff1681526020019250505060405180910390a1600190505b919050565b600060005490505b9056',
        };

        const contractABI = [{'constant': false,'inputs': [],'name': 'setcompeltereturn','outputs': [{'name': '_newValue','type': 'uint256'},{'name': '_sender','type': 'address'}],'payable': false,'type': 'function'},{'constant': false,'inputs': [{'name': '_value','type': 'uint256'}],'name': 'set','outputs': [{'name': '','type': 'bool'}],'payable': false,'type': 'function'},{'constant': false,'inputs': [],'name': 'get','outputs': [{'name': 'storeValue','type': 'uint256'}],'payable': false,'type': 'function'},{'anonymous':false,'inputs':[{'indexed':false,'name':'_newValue','type':'uint256'},{'indexed':false,'name':'_sender','type':'address'}],'name':'SetComplete','type':'event'}]; // eslint-disable-line

        eth.sendTransaction(testContractTransaction, (error, result) => {
          assert.equal(error, null);
          assert.equal(typeof result, 'string');
          assert.equal(util.getBinarySize(result), 66);

          setTimeout(() => {
            eth.getTransactionReceipt(result, (receiptError, receipt) => {
              assert.equal(receiptError, null);
              assert.equal(typeof receipt, 'object');

              assert.equal(util.getBinarySize(receipt.transactionHash), 66);
              assert.equal(receipt.transactionIndex.toNumber(10) >= 0, true);
              assert.equal(receipt.blockNumber.toNumber(10) >= 0, true);
              assert.equal(receipt.cumulativeGasUsed.toNumber(10) >= 0, true);
              assert.equal(receipt.gasUsed.toNumber(10) >= 0, true);
              assert.equal(Array.isArray(receipt.logs), true);
              assert.equal(typeof receipt.contractAddress, 'string');

              const uintValue = 350000;
              const setMethodTransaction = {
                from: accounts[0],
                to: receipt.contractAddress,
                gas: 3000000,
                data: abi.encodeMethod(contractABI[1], [uintValue]),
              };

              eth.sendTransaction(setMethodTransaction, (setMethodError, setMethodTx) => {
                assert.equal(setMethodError, null);
                assert.equal(typeof setMethodTx, 'string');
                assert.equal(util.getBinarySize(setMethodTx), 66);

                setTimeout(() => {
                  const callMethodTransaction = {
                    to: receipt.contractAddress,
                    data: abi.encodeMethod(contractABI[2], []),
                  };

                  eth.call(callMethodTransaction, (callError, callResult) => { // eslint-disable-line
                    assert.equal(setMethodError, null);
                    const decodedUint = abi.decodeMethod(contractABI[2], callResult);

                    assert.equal(decodedUint[0].toNumber(10), uintValue);

                    eth.getCode(receipt.contractAddress, 'latest', (codeError, codeResult) => {
                      assert.equal(codeError, null);
                      assert.equal(typeof codeResult, 'string');

                      done();
                    });
                  });
                }, 400);
              });
            });
          }, 1000);
        });
      });
    });

    it('should function while deploy, use contract via eth_call, eth_getCode with debug, logger', (done) => {
      const eth = new Eth(provider, { debug: true, logger: { log: () => {} }, jsonSpace: 2 }); // eslint-disable-line

      eth.accounts((accountsError, accounts) => {
        assert.equal(accountsError, null);
        assert.equal(typeof accounts, 'object');

        const testContractTransaction = {
          from: accounts[0],
          gas: 3000000,
          data: '606060405234610000575b61016a806100186000396000f360606040526000357c010000000000000000000000000000000000000000000000000000000090048063119c56bd1461004e57806360fe47b11461008e5780636d4ce63c146100c1575b610000565b346100005761005b6100e4565b604051808381526020018273ffffffffffffffffffffffffffffffffffffffff1681526020019250505060405180910390f35b34610000576100a960048080359060200190919050506100f5565b60405180821515815260200191505060405180910390f35b34610000576100ce61015f565b6040518082815260200191505060405180910390f35b60006000610d7d91503390505b9091565b6000816000819055507f10e8e9bc5a1bde3dd6bb7245b52503fcb9d9b1d7c7b26743f82c51cc7cce917d60005433604051808381526020018273ffffffffffffffffffffffffffffffffffffffff1681526020019250505060405180910390a1600190505b919050565b600060005490505b9056',
        };

        const contractABI = [{'constant': false,'inputs': [],'name': 'setcompeltereturn','outputs': [{'name': '_newValue','type': 'uint256'},{'name': '_sender','type': 'address'}],'payable': false,'type': 'function'},{'constant': false,'inputs': [{'name': '_value','type': 'uint256'}],'name': 'set','outputs': [{'name': '','type': 'bool'}],'payable': false,'type': 'function'},{'constant': false,'inputs': [],'name': 'get','outputs': [{'name': 'storeValue','type': 'uint256'}],'payable': false,'type': 'function'},{'anonymous':false,'inputs':[{'indexed':false,'name':'_newValue','type':'uint256'},{'indexed':false,'name':'_sender','type':'address'}],'name':'SetComplete','type':'event'}]; // eslint-disable-line

        eth.sendTransaction(testContractTransaction, (error, result) => {
          assert.equal(error, null);
          assert.equal(typeof result, 'string');
          assert.equal(util.getBinarySize(result), 66);

          setTimeout(() => {
            eth.getTransactionReceipt(result, (receiptError, receipt) => {
              assert.equal(receiptError, null);
              assert.equal(typeof receipt, 'object');

              assert.equal(util.getBinarySize(receipt.transactionHash), 66);
              assert.equal(receipt.transactionIndex.toNumber(10) >= 0, true);
              assert.equal(receipt.blockNumber.toNumber(10) >= 0, true);
              assert.equal(receipt.cumulativeGasUsed.toNumber(10) >= 0, true);
              assert.equal(receipt.gasUsed.toNumber(10) >= 0, true);
              assert.equal(Array.isArray(receipt.logs), true);
              assert.equal(typeof receipt.contractAddress, 'string');

              const uintValue = 350000;
              const setMethodTransaction = {
                from: accounts[0],
                to: receipt.contractAddress,
                gas: 3000000,
                data: abi.encodeMethod(contractABI[1], [uintValue]),
              };

              eth.sendTransaction(setMethodTransaction, (setMethodError, setMethodTx) => {
                assert.equal(setMethodError, null);
                assert.equal(typeof setMethodTx, 'string');
                assert.equal(util.getBinarySize(setMethodTx), 66);

                setTimeout(() => {
                  const callMethodTransaction = {
                    to: receipt.contractAddress,
                    data: abi.encodeMethod(contractABI[2], []),
                  };

                  eth.call(callMethodTransaction, (callError, callResult) => { // eslint-disable-line
                    assert.equal(setMethodError, null);
                    const decodedUint = abi.decodeMethod(contractABI[2], callResult);

                    assert.equal(decodedUint[0].toNumber(10), uintValue);

                    eth.getCode(receipt.contractAddress, 'latest', (codeError, codeResult) => {
                      assert.equal(codeError, null);
                      assert.equal(typeof codeResult, 'string');

                      done();
                    });
                  });
                }, 400);
              });
            });
          }, 1000);
        });
      });
    });
  });
});
