/* eslint-disable */

const assert = require('chai').assert;
const format = require('../index.js');
const BN = require('bn.js');
const schema = require('ethjs-schema');

describe('test ethjs-format object', () => {
  describe('test formatQuantity', () => {
    it('should encode normally', () => {
      const bignum = new BN(45333);
      const num = 327243762;
      const stringNumber = '238428237842';
      const noPrefixHexNumber = '21D21A2';
      const prefixHexNumber = '0x21d21a2';
      const nullNumber = null;
      const undefinedNumber = undefined;
      const emptyString = '';
      const zeroNumber = 0;
      const justPrefix = '0';
      const floatNumber = '238428.237842';

      assert.equal(format.formatQuantity(emptyString, true), '0x0');
      assert.equal(format.formatQuantity(zeroNumber, true), '0x0');
      assert.equal(format.formatQuantity(nullNumber, true), null);
      assert.equal(format.formatQuantity(undefinedNumber, true), undefinedNumber);
      assert.equal(format.formatQuantity(bignum, true), '0xB115'.toLowerCase());
      assert.equal(format.formatQuantity(num, true), '0x138157F2'.toLowerCase());
      assert.equal(format.formatQuantity(stringNumber, true), '0x37836E3012'.toLowerCase());
      assert.equal(format.formatQuantity(noPrefixHexNumber, true), '0x21d21a2'.toLowerCase());
      assert.equal(format.formatQuantity(prefixHexNumber, true), '0x21d21a2'.toLowerCase());
      assert.equal(format.formatQuantity(justPrefix, true), '0x0');
      assert.equal(format.formatQuantity('0x10', true), '0x10');

      // padding tests for encoding
      assert.equal(format.formatQuantity(new BN(1, 10), true, true), '0x01');
      assert.equal(format.formatQuantity(new BN(11, 10), true, true), '0x0b');
      assert.equal(format.formatQuantity(new BN(0, 10), true, true), '0x00');

      try {
        format.formatQuantity(floatNumber, true);
      } catch (error) {
        assert.equal(typeof error, 'object');
      }
    });

    it('test format object EthSyncing', () => {

      const objEth = {
        startingBlock: "0x57840CC2C",
        currentBlock: "0x57840CC2C",
        highestBlock: "0x57840CC2C",
      };

      assert.deepEqual(format.formatObject('Boolean|EthSyncing', objEth).startingBlock.toNumber(10) > 0, true);
    });

    it('test is required keys are filled, should throw', () => {
      const encodedSendTransactionObject = {
        'to': '0xd46e8dd67c5d32be8058bb8eb970870f07244567',
        'gas': '0x76c0', // 30400,
        'gasPrice': '0x9184e72a000', // 10000000000000
        'value': '0x9184e72a', // 2441406250
        'data': '0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675'
      };

      assert.throws(() => {
        format.formatObject('SendTransaction', encodedSendTransactionObject);
      }, Error);
    });

    it('test format array', () => {
      assert.deepEqual(format.formatArray('Array|DATA', ['0x']), ['0x']);
    });

    it('test format array', () => {
      assert.deepEqual(format.formatArray('FilterChange', ['0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55']), ['0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55']);
    });

    it('should decode normally', () => {
      const prefixHexNumber = '0x57840CC2C';
      const noPrefixHexNumber = '21D21A2';
      const largeHexNumber = '0x2386F26FC0FFFF';
      const bignum = new BN(45333);
      const num = 327243762;
      const stringNumber = '238428237842';
      const nullNumber = null;
      const undefinedNumber = undefined;
      const emptyString = '';
      const zeroNumber = 0;
      const justPrefix = '0';
      const floatNumber = '238428.237842';

      const r1 = format.formatQuantity(prefixHexNumber, false).toString(10);
      const r2 = format.formatQuantity(noPrefixHexNumber, false).toString(10);
      const r3 = format.formatQuantity(largeHexNumber, false).toString(10);
      const r4 = format.formatQuantity(bignum, false).toString(10);
      const r5 = format.formatQuantity(num, false).toString(10);
      const r6 = format.formatQuantity(stringNumber, false).toString(10);
      const r7 = format.formatQuantity(nullNumber, false);
      const r8 = format.formatQuantity(emptyString, false).toString(10);
      const r9 = format.formatQuantity(zeroNumber, false).toString(10);
      const r10 = format.formatQuantity(justPrefix, false).toString(10);

      assert.equal(r1, '23492348972');
      assert.equal(r2, '35463586');
      assert.equal(r3, '9999999999999999');
      assert.equal(r4, '45333');
      assert.equal(r5, '327243762');
      assert.equal(r6, '238428237842');
      assert.equal(r7, null);
      assert.equal(r8, '0');
      assert.equal(r9, '0');
      assert.equal(r10, '0');
      assert.equal(format.formatQuantity(undefinedNumber, false), undefinedNumber);

      try {
        format.formatQuantity(floatNumber, false);
      } catch (error) {
        assert.equal(typeof error, 'object');
      }
    });
  });

  describe('test formatQuantityOrTag', () => {
    it('should encode normally', () => {
      const bignum = new BN(45333);
      const num = 327243762;
      const stringNumber = '238428237842';
      const noPrefixHexNumber = '21D21A2';
      const prefixHexNumber = '0x21d21a2';
      const nullNumber = null;
      const undefinedNumber = undefined;
      const pendingTag = 'pending';
      const latestTag = 'pending';
      const earliestTag = 'earliest';

      assert.equal(format.formatQuantityOrTag(pendingTag, true), pendingTag);
      assert.equal(format.formatQuantityOrTag(latestTag, true), latestTag);
      assert.equal(format.formatQuantityOrTag(earliestTag, true), earliestTag);

      assert.equal(format.formatQuantityOrTag(nullNumber, true), null);
      assert.equal(format.formatQuantityOrTag(undefinedNumber, true), undefinedNumber);
      assert.equal(format.formatQuantityOrTag(bignum, true), '0xB115'.toLowerCase());
      assert.equal(format.formatQuantityOrTag(num, true), '0x138157F2'.toLowerCase());
      assert.equal(format.formatQuantityOrTag(stringNumber, true), '0x37836E3012'.toLowerCase());
      assert.equal(format.formatQuantityOrTag(noPrefixHexNumber, true), '0x21d21a2'.toLowerCase());
      assert.equal(format.formatQuantityOrTag(prefixHexNumber, true), '0x21d21a2'.toLowerCase());
    });

    it('should decode normally', () => {
      const prefixHexNumber = '0x57840CC2C';
      const noPrefixHexNumber = '21D21A2';
      const largeHexNumber = '0x2386F26FC0FFFF';
      const bignum = new BN(45333);
      const num = 327243762;
      const stringNumber = '238428237842';
      const nullNumber = null;
      const undefinedNumber = undefined;
      const pendingTag = 'pending';
      const latestTag = 'pending';
      const earliestTag = 'earliest';

      assert.equal(format.formatQuantityOrTag(pendingTag, false), pendingTag);
      assert.equal(format.formatQuantityOrTag(latestTag, false), latestTag);
      assert.equal(format.formatQuantityOrTag(earliestTag, false), earliestTag);

      const r1 = format.formatQuantityOrTag(prefixHexNumber, false).toString(10);
      const r2 = format.formatQuantityOrTag(noPrefixHexNumber, false).toString(10);
      const r3 = format.formatQuantityOrTag(largeHexNumber, false).toString(10);
      const r4 = format.formatQuantityOrTag(bignum, false).toString(10);
      const r5 = format.formatQuantityOrTag(num, false).toString(10);
      const r6 = format.formatQuantityOrTag(stringNumber, false).toString(10);
      const r7 = format.formatQuantityOrTag(nullNumber, false);

      assert.equal(r1, '23492348972');
      assert.equal(r2, '35463586');
      assert.equal(r3, '9999999999999999');
      assert.equal(r4, '45333');
      assert.equal(r5, '327243762');
      assert.equal(r6, '238428237842');
      assert.equal(r7, null);
      assert.equal(format.formatQuantityOrTag(undefinedNumber, false), undefinedNumber);
    });
  });

  describe('test formatObject', () => {
    it('receipt should decode normally', () => {
      const encodedReceiptObject = {
        transactionHash: '0xb903239f8543d04b5dc1ba6579132b143087c68db1b2168786408fcbce568238',
        transactionIndex: '0x1', // 1
        blockNumber: '0xb', // 11
        blockHash: '0xc6ef2fc5426d6ad6fd9e2a26abeab0aa2411b7ab17f30a99d3cb96aed1d1055b',
        cumulativeGasUsed: '0x33bc', // 13244
        gasUsed: '0x4dc', // 1244
        contractAddress: '0xb60e8dd61c5d32be8058bb8eb970870f07233155', // or null, if none was created
        logs: [{
          logIndex: '0x1', // 1
          blockNumber: '0x1b4', // 436
          blockHash: '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d31',
          transactionHash: '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55',
          transactionIndex: '0x0', // 0
          address: '0x16c5785ac562ff41e2dcfdf829c5a142f1fccd7d',
          data: '0x0000000000000000000000000000000000000000000000000000000000000000',
          topics: ['0x59ebeb90bc63057b6515673c3ecf9438e5058bca0f92585014eced636878c9a5'],
        }],
      };

      const decodedObject = format.formatObject('Receipt', encodedReceiptObject, false);

      assert.equal(decodedObject.transactionHash, '0xb903239f8543d04b5dc1ba6579132b143087c68db1b2168786408fcbce568238');
      assert.equal(decodedObject.transactionIndex.toString(10), '1');
      assert.equal(decodedObject.blockNumber.toString(10), '11');
      assert.equal(decodedObject.gasUsed.toString(10), '1244');
      assert.equal(decodedObject.cumulativeGasUsed.toString(10), '13244');
      assert.equal(decodedObject.contractAddress, '0xb60e8dd61c5d32be8058bb8eb970870f07233155');
      assert.equal(decodedObject.blockHash, '0xc6ef2fc5426d6ad6fd9e2a26abeab0aa2411b7ab17f30a99d3cb96aed1d1055b');
      assert.equal(Array.isArray(decodedObject.logs), true);
      assert.equal(Array.isArray(decodedObject.logs[0].topics), true);

      assert.equal(decodedObject.logs[0].data, '0x0000000000000000000000000000000000000000000000000000000000000000');
      assert.equal(decodedObject.logs[0].data, '0x0000000000000000000000000000000000000000000000000000000000000000');
      assert.equal(decodedObject.logs[0].address, '0x16c5785ac562ff41e2dcfdf829c5a142f1fccd7d');
      assert.equal(decodedObject.logs[0].blockHash, '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d31');
      assert.equal(decodedObject.logs[0].transactionHash, '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55');

      assert.equal(decodedObject.logs[0].logIndex.toString(10), '1');
      assert.equal(decodedObject.logs[0].blockNumber.toString(10), '436');
      assert.equal(decodedObject.logs[0].transactionIndex.toString(10), '0');
    });

    it('transaction object should decode normally', () => {
      const encodedTransactionObject = {
        hash: '0xc6ef2fc5426d6ad6fd9e2a26abeab0aa2411b7ab17f30a99d3cb96aed1d1055b',
        nonce: '0x',
        blockHash: '0xbeab0aa2411b7ab17f30a99d3cb9c6ef2fc5426d6ad6fd9e2a26a6aed1d1055b',
        blockNumber: '0x15df', // 5599
        transactionIndex: '0x1', // 1
        from: '0x407d73d8a49eeb85d32cf465507dd71d507100c1',
        to: '0x85e43d8a49eeb85d32cf465507dd71d507100c12',
        value: '0x7f110', // 520464
        gas: '0x7f110', // 520464
        gasPrice: '0x9184e72a000',
        input: '0x603880600c6000396000f300603880600c6000396000f3603880600c6000396000f360',
      };

      const decodedObject = format.formatObject('Transaction', encodedTransactionObject, false);

      assert.equal(decodedObject.hash, '0xc6ef2fc5426d6ad6fd9e2a26abeab0aa2411b7ab17f30a99d3cb96aed1d1055b');
      assert.equal(decodedObject.nonce.toString(10), '0');
      assert.equal(decodedObject.blockHash, '0xbeab0aa2411b7ab17f30a99d3cb9c6ef2fc5426d6ad6fd9e2a26a6aed1d1055b');
      assert.equal(decodedObject.blockNumber.toString(10), '5599');
      assert.equal(decodedObject.transactionIndex.toString(10), '1');
      assert.equal(decodedObject.gasPrice.toString(10), '10000000000000');
      assert.equal(decodedObject.value.toString(10), '520464');
      assert.equal(decodedObject.gas.toString(10), '520464');
      assert.equal(decodedObject.from, '0x407d73d8a49eeb85d32cf465507dd71d507100c1');
      assert.equal(decodedObject.to, '0x85e43d8a49eeb85d32cf465507dd71d507100c12');
      assert.equal(decodedObject.input, '0x603880600c6000396000f300603880600c6000396000f3603880600c6000396000f360');

      const encodedTransactionObjectWithEmptyAddress = {
        hash: '0xc6ef2fc5426d6ad6fd9e2a26abeab0aa2411b7ab17f30a99d3cb96aed1d1055b',
        nonce: '0x',
        blockHash: '0xbeab0aa2411b7ab17f30a99d3cb9c6ef2fc5426d6ad6fd9e2a26a6aed1d1055b',
        blockNumber: '0x15df', // 5599
        transactionIndex: '0x1', // 1
        from: '0x0',
        to: '0x85e43d8a49eeb85d32cf465507dd71d507100c12',
        value: '0x7f110', // 520464
        gas: '0x7f110', // 520464
        gasPrice: '0x9184e72a000',
        input: '0x603880600c6000396000f300603880600c6000396000f3603880600c6000396000f360',
      };

      const decodedObject2 = format.formatObject('Transaction', encodedTransactionObjectWithEmptyAddress, false);
      assert.equal(decodedObject2.from, '0x0');
    });

    it('should decode Block object normally', () => {
      const encodedTransactionObject = {
        'hash':'0xc6ef2fc5426d6ad6fd9e2a26abeab0aa2411b7ab17f30a99d3cb96aed1d1055b',
        'nonce':'0x',
        'blockHash': '0xbeab0aa2411b7ab17f30a99d3cb9c6ef2fc5426d6ad6fd9e2a26a6aed1d1055b',
        'blockNumber': '0x15df', // 5599
        'transactionIndex':  '0x1', // 1
        'from':'0x407d73d8a49eeb85d32cf465507dd71d507100c1',
        'to':'0x85e43d8a49eeb85d32cf465507dd71d507100c12',
        'value':'0x7f110', // 520464
        'gas': '0x7f110', // 520464
        'gasPrice':'0x9184e72a000',
        'input':'0x603880600c6000396000f300603880600c6000396000f3603880600c6000396000f360',
      };

      const encodedBlockObject = {
        'number': '0x1b4', // 436
        'hash': '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d32',
        'parentHash': '0x9646252be9520f6e71339a8df9c55e4d7619deeb018d2a3f2d21fc165dde5eb5',
        'nonce': '0xe04d296d2460cfb8472af2c5fd05b5a214109c25688d3704aed5484f9a7792f2',
        'sha3Uncles': '0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347',
        'logsBloom': '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d32',
        'transactionsRoot': '0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421',
        'stateRoot': '0xd5855eb08b3387c0af375e9cdb6acfc05eb8f519e419b874b6ff2ffda7ed1dff',
        'miner': '0x4e65fda2159562a496f9f3522f89122a3088497a',
        'difficulty': '0x027f07', // 163591
        'totalDifficulty':  '0x027f07', // 163591
        'extraData': '0x0000000000000000000000000000000000000000000000000000000000000000',
        'size':  '0x027f07', // 163591
        'gasLimit': '0x9f759', // 653145
        'gasUsed': '0x9f759', // 653145
        'timestamp': '0x54e34e8e', // 1424182926
        'transactions': ['0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d32', encodedTransactionObject],
        'uncles': ['0x9646252be9520f6e71339a8df9c55e4d7619deeb018d2a3f2d21fc165dde5eb5', '0x9646252be9520f6e71339a8df9c55e4d7619deeb018d2a3f2d21fc165dde5eb5'],
      };

      const decodedObject = format.formatObject('Block', encodedBlockObject, false);

      assert.equal(decodedObject.hash, '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d32');
      assert.equal(decodedObject.parentHash, '0x9646252be9520f6e71339a8df9c55e4d7619deeb018d2a3f2d21fc165dde5eb5');
      assert.equal(decodedObject.nonce, '0xe04d296d2460cfb8472af2c5fd05b5a214109c25688d3704aed5484f9a7792f2');
      assert.equal(decodedObject.sha3Uncles, '0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347');
      assert.equal(decodedObject.logsBloom, '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d32');
      assert.equal(decodedObject.transactionsRoot, '0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421');
      assert.equal(decodedObject.stateRoot, '0xd5855eb08b3387c0af375e9cdb6acfc05eb8f519e419b874b6ff2ffda7ed1dff');
      assert.equal(decodedObject.miner, '0x4e65fda2159562a496f9f3522f89122a3088497a');

      assert.equal(decodedObject.number.toString(10), '436');
      assert.equal(decodedObject.difficulty.toString(10), '163591');
      assert.equal(decodedObject.totalDifficulty.toString(10), '163591');
      assert.equal(decodedObject.size.toString(10), '163591');
      assert.equal(decodedObject.gasLimit.toString(10), '653145');
      assert.equal(decodedObject.gasUsed.toString(10), '653145');
      assert.equal(decodedObject.extraData, '0x0000000000000000000000000000000000000000000000000000000000000000');

      assert.equal(Array.isArray(decodedObject.transactions), true);
      assert.equal(Array.isArray(decodedObject.uncles), true);

      assert.equal(decodedObject.transactions[0], '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d32');
      assert.equal(decodedObject.uncles[0], '0x9646252be9520f6e71339a8df9c55e4d7619deeb018d2a3f2d21fc165dde5eb5');

      assert.equal(decodedObject.transactions[1].hash, '0xc6ef2fc5426d6ad6fd9e2a26abeab0aa2411b7ab17f30a99d3cb96aed1d1055b');
      assert.equal(decodedObject.transactions[1].nonce.toString(10), '0');
      assert.equal(decodedObject.transactions[1].blockHash, '0xbeab0aa2411b7ab17f30a99d3cb9c6ef2fc5426d6ad6fd9e2a26a6aed1d1055b');
      assert.equal(decodedObject.transactions[1].blockNumber.toString(10), '5599');
      assert.equal(decodedObject.transactions[1].transactionIndex.toString(10), '1');
      assert.equal(decodedObject.transactions[1].gasPrice.toString(10), '10000000000000');
      assert.equal(decodedObject.transactions[1].value.toString(10), '520464');
      assert.equal(decodedObject.transactions[1].gas.toString(10), '520464');
      assert.equal(decodedObject.transactions[1].from, '0x407d73d8a49eeb85d32cf465507dd71d507100c1');
      assert.equal(decodedObject.transactions[1].to, '0x85e43d8a49eeb85d32cf465507dd71d507100c12');
      assert.equal(decodedObject.transactions[1].input, '0x603880600c6000396000f300603880600c6000396000f3603880600c6000396000f360');
    });

    it('should encode Filter object normally', () => {
      const decodedFilterObject = {
        fromBlock: 89886779,
        toBlock: 'latest',
        'address': '0x8888f1f195afa192cfee860698584c030f4c9db1',
        'topics': ['0x000000000000000000000000a94f5374fce5edbc8e2a8697c15331677e6ebf0b', null, ['0x000000000000000000000000a94f5374fce5edbc8e2a8697c15331677e6ebf0b', '0x000000000000000000000000aff3454fce5edbc8cca8697c15331677e6ebccc']],
      };

      const encodedObject = format.formatObject('Filter', decodedFilterObject, true);

      assert.equal(encodedObject.fromBlock, '0x55B903B'.toLowerCase());
      assert.equal(encodedObject.toBlock, 'latest');
      assert.equal(Array.isArray(encodedObject.topics), true);
    });

    it('should decode send transaction object normally', () => {
      const encodedSendTransactionObject = {
        'from': '0xb60e8dd61c5d32be8058bb8eb970870f07233155',
        'to': '0xd46e8dd67c5d32be8058bb8eb970870f07244567',
        'gas': '0x76c0', // 30400,
        'gasPrice': '0x9184e72a000', // 10000000000000
        'value': '0x9184e72a', // 2441406250
        'data': '0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675'
      };

      const decodedObject = format.formatObject('SendTransaction', encodedSendTransactionObject, false);

      assert.equal(decodedObject.from, '0xb60e8dd61c5d32be8058bb8eb970870f07233155');
      assert.equal(decodedObject.to, '0xd46e8dd67c5d32be8058bb8eb970870f07244567');
      assert.equal(decodedObject.data, '0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675');

      assert.equal(decodedObject.gas.toString(10), '30400');
      assert.equal(decodedObject.gasPrice.toString(10), '10000000000000');
      assert.equal(decodedObject.value.toString(10), '2441406250');
    });

    it('should encode SendTransaction object normally', () => {
      const decodedSendTransactionObject_1 = {
        'from': '0xb60e8dd61c5d32be8058bb8eb970870f07233155',
        'to': '0xd46e8dd67c5d32be8058bb8eb970870f07244567',
        'gas': '30400', // 30400,
        'gasPrice': '10000000000000', // 10000000000000
        'value': '2441406250', // 2441406250
        'data': '0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675'
      };

      const encodedObject_1 = format.formatObject('SendTransaction', decodedSendTransactionObject_1, true);

      assert.equal(encodedObject_1.from, '0xb60e8dd61c5d32be8058bb8eb970870f07233155');
      assert.equal(encodedObject_1.to, '0xd46e8dd67c5d32be8058bb8eb970870f07244567');
      assert.equal(encodedObject_1.data, '0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675');

      assert.equal(encodedObject_1.gas, '0x76c0');
      assert.equal(encodedObject_1.gasPrice, '0x9184e72a000');
      assert.equal(encodedObject_1.value, '0x9184e72a');

      const decodedSendTransactionObject_2 = {
        'from': '0xb60e8dd61c5d32be8058bb8eb970870f07233155',
        'to': '0xd46e8dd67c5d32be8058bb8eb970870f07244567',
        'gas': new BN('30400'), // 30400,
        'gasPrice': '10000000000000', // 10000000000000
        'data': '0x',
      };

      const encodedObject_2 = format.formatObject('SendTransaction', decodedSendTransactionObject_2, true);

      assert.equal(encodedObject_2.from, '0xb60e8dd61c5d32be8058bb8eb970870f07233155');
      assert.equal(encodedObject_2.to, '0xd46e8dd67c5d32be8058bb8eb970870f07244567');
      assert.equal(encodedObject_2.data, '0x');

      assert.equal(encodedObject_2.gas, '0x76c0');
      assert.equal(encodedObject_2.gasPrice, '0x9184e72a000');

      const decodedSendTransactionObject_3 = {
        'from': '0xb60e8dd61c5d32be8058bb8eb970870f07233155',
        'gas': 30400, // 30400,
        'gasPrice': '10000000000000', // 10000000000000
        'data': '0x',
      };

      const encodedObject_3 = format.formatObject('SendTransaction', decodedSendTransactionObject_3, true);

      assert.equal(encodedObject_3.from, '0xb60e8dd61c5d32be8058bb8eb970870f07233155');
      assert.equal(encodedObject_3.data, '0x');

      assert.equal(encodedObject_3.gas, '0x76c0');
      assert.equal(encodedObject_3.gasPrice, '0x9184e72a000');

      const decodedSendTransactionObject_4 = {
        'from': '0xb60e8dd61c5d32be8058bb8eb970870f07233155',
        'gas': 348978973, // 30400,
        'data': '0x',
        'gasPrice': new BN(10000000000000), // 10000000000000
      };

      const encodedObject_4 = format.formatObject('SendTransaction', decodedSendTransactionObject_4, true);

      assert.equal(encodedObject_4.from, '0xb60e8dd61c5d32be8058bb8eb970870f07233155');
      assert.equal(encodedObject_4.data, '0x');

      assert.equal(encodedObject_4.gas, '0x14CCFF1D'.toLowerCase());
      assert.equal(encodedObject_4.gasPrice, '0x9184e72a000');

      const decodedSendTransactionObject_5 = {
        'from': '0xd46e8dd67c5d32be8058bb8eb970870f07244567',
        'gas': new BN(10000000000000), // 30400,
        'data': '0x',
        'gasPrice': 348978973, // 10000000000000
      };

      const encodedObject_5 = format.formatObject('SendTransaction', decodedSendTransactionObject_5, true);

      assert.equal(encodedObject_5.from, '0xd46e8dd67c5d32be8058bb8eb970870f07244567');
      assert.equal(encodedObject_4.data, '0x');

      assert.equal(encodedObject_5.gas, '0x9184e72a000');
      assert.equal(encodedObject_5.gasPrice, '0x14CCFF1D'.toLowerCase());
    });
  });

  describe('test decode of getBlockByNumber', () => {
    const payload = JSON.parse('{"author":"0x61c808d82a3ac53231750dadc13c777b59310bd9","difficulty":"0x4cc38f1df101","extraData":"0xe4b883e5bda9e7a59ee4bb99e9b1bc","gasLimit":"0x3d1e65","gasUsed":"0xb238","hash":"0x5d336fc52ebd4c32dec4fd1a82058521f8d43f76e0c47a6540577253fcc5eba4","logsBloom":"0x00000000000000020000000000020000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000202000000000000000000000000000001000000000040000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000001000000000000000000000000000000000","miner":"0x61c808d82a3ac53231750dadc13c777b59310bd9","mixHash":"0xf3bd964ff1ba978efad5538e855b6d76a5e60fd55d556893abf5dcecd5cd339b","nonce":"0x128d840a2e9b7ba2","number":"0x2a91ec","parentHash":"0x31ba04f14e28e0ae36ccb69d440adcf67bead1648e43341186fc86324f76db3a","receiptsRoot":"0xa09bf3f2b0f9d7fb8dc03c6d40d24903c336ce1d06803acc75337a4518e1ba23","sealFields":["0xf3bd964ff1ba978efad5538e855b6d76a5e60fd55d556893abf5dcecd5cd339b","0x128d840a2e9b7ba2"],"sha3Uncles":"0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347","size":"0x2c3","stateRoot":"0xeac15b6137f875f45dade12633005003898c73c6f8b52c024a70a5746c7c093b","timestamp":"0x584d8cfd","totalDifficulty":"0x582c5ac155a130807","transactions":["0xc2c139851af8c356039c717be5741604bc01231968c09db284a6d809a74c67de"],"transactionsRoot":"0x4ef878dcdafa2569903739be44e10ed89548114ad0c8078b6b7619ca4dca12c7","uncles":[]}');

    const formattedData = format.formatOutputs('eth_getBlockByHash', payload);

    assert.equal(typeof formattedData.receiptsRoot, 'string');
    assert.equal(formattedData.miner, '0x61c808d82a3ac53231750dadc13c777b59310bd9');
    assert.equal(formattedData.author, '0x61c808d82a3ac53231750dadc13c777b59310bd9');
    assert.equal(formattedData.extraData, '0xe4b883e5bda9e7a59ee4bb99e9b1bc');
    assert.equal(typeof formattedData.number, 'object');
    assert.equal(typeof formattedData.nonce, 'string');
    assert.equal(typeof formattedData.gasLimit, 'object');
    assert.equal(typeof formattedData.size, 'object');
    assert.equal(typeof formattedData.timestamp, 'object');
    assert.equal(Array.isArray(formattedData.uncles), true);
    assert.equal(formattedData.transactionsRoot, '0x4ef878dcdafa2569903739be44e10ed89548114ad0c8078b6b7619ca4dca12c7');
  });

  describe('test format', () => {
    it('should encode QUANTITY normally', () => {
      const bignum = new BN(45333);
      const num = 327243762;
      const stringNumber = '238428237842';
      const noPrefixHexNumber = '21D21A2';
      const prefixHexNumber = '0x21d21a2';
      const nullNumber = null;
      const undefinedNumber = undefined;

      assert.equal(format.format('Q', nullNumber, true), null);
      assert.equal(format.format('Q', undefinedNumber, true), undefinedNumber);
      assert.equal(format.format('Q', bignum, true), '0xB115'.toLowerCase());
      assert.equal(format.format('Q', num, true), '0x138157F2'.toLowerCase());
      assert.equal(format.format('Q', stringNumber, true), '0x37836E3012'.toLowerCase());
      assert.equal(format.format('Q', noPrefixHexNumber, true), '0x21d21a2'.toLowerCase());
      assert.equal(format.format('Q', prefixHexNumber, true), '0x21d21a2'.toLowerCase());

      assert.equal(format.format('QP', new BN(0), true), '0x00');
      assert.equal(format.format('QP', new BN(1), true), '0x01');
      assert.equal(format.format('QP', new BN(10), true), '0x0a');

      assert.equal(format.format('QP', nullNumber, true), null);
      assert.equal(format.format('QP', undefinedNumber, true), undefinedNumber);
      assert.equal(format.format('QP', bignum, true), '0xB115'.toLowerCase());
      assert.equal(format.format('QP', num, true), '0x138157F2'.toLowerCase());
      assert.equal(format.format('QP', stringNumber, true), '0x37836E3012'.toLowerCase());
      assert.equal(format.format('QP', noPrefixHexNumber, true), '0x021d21a2'.toLowerCase());
      assert.equal(format.format('QP', prefixHexNumber, true), '0x021d21a2'.toLowerCase());
    });

    it('should decode QUANTITY normally', () => {
      const prefixHexNumber = '0x57840CC2C';
      const noPrefixHexNumber = '21D21A2';
      const largeHexNumber = '0x2386F26FC0FFFF';
      const bignum = new BN(45333);
      const num = 327243762;
      const stringNumber = '238428237842';
      const nullNumber = null;
      const undefinedNumber = undefined;
      const emptyString = '';
      const zeroNumber = 0;

      const r1 = format.format('Q', prefixHexNumber, false).toString(10);
      const r2 = format.format('Q', noPrefixHexNumber, false).toString(10);
      const r3 = format.format('Q', largeHexNumber, false).toString(10);
      const r4 = format.format('Q', bignum, false).toString(10);
      const r5 = format.format('Q', num, false).toString(10);
      const r6 = format.format('Q', stringNumber, false).toString(10);
      const r7 = format.format('Q', nullNumber, false);
      const r8 = format.format('Q', emptyString, false).toString(10);
      const r9 = format.format('Q', zeroNumber, false).toString(10);

      assert.throws(() => format.format('Q', '-10', false), Error);
      assert.equal(r1, '23492348972');
      assert.equal(r2, '35463586');
      assert.equal(r3, '9999999999999999');
      assert.equal(r4, '45333');
      assert.equal(r5, '327243762');
      assert.equal(r6, '238428237842');
      assert.equal(r7, null);
      assert.equal(r8, '0');
      assert.equal(r9, '0');
      assert.equal(format.formatQuantity(undefinedNumber, false), undefinedNumber);
    });

    it('should encode QUANTITY|TAG normally', () => {
      const bignum = new BN(45333);
      const num = 327243762;
      const stringNumber = '238428237842';
      const noPrefixHexNumber = '21D21A2';
      const prefixHexNumber = '0x21d21a2';
      const nullNumber = null;
      const undefinedNumber = undefined;
      const pendingTag = 'pending';
      const latestTag = 'pending';
      const earliestTag = 'earliest';

      assert.equal(format.format('Q|T', pendingTag, true), pendingTag);
      assert.equal(format.format('Q|T', latestTag, true), latestTag);
      assert.equal(format.format('Q|T', earliestTag, true), earliestTag);

      assert.equal(format.format('Q|T', nullNumber, true), null);
      assert.equal(format.format('Q|T', undefinedNumber, true), undefinedNumber);
      assert.equal(format.format('Q|T', bignum, true), '0xB115'.toLowerCase());
      assert.equal(format.format('Q|T', num, true), '0x138157F2'.toLowerCase());
      assert.equal(format.format('Q|T', stringNumber, true), '0x37836E3012'.toLowerCase());
      assert.equal(format.format('Q|T', noPrefixHexNumber, true), '0x21d21a2'.toLowerCase());
      assert.equal(format.format('Q|T', prefixHexNumber, true), '0x21d21a2'.toLowerCase());
    });

    it('should decode QUANTITY|TAG normally', () => {
      const prefixHexNumber = '0x57840CC2C';
      const noPrefixHexNumber = '21D21A2';
      const largeHexNumber = '0x2386F26FC0FFFF';
      const bignum = new BN(45333);
      const num = 327243762;
      const stringNumber = '238428237842';
      const nullNumber = null;
      const undefinedNumber = undefined;
      const pendingTag = 'pending';
      const latestTag = 'pending';
      const earliestTag = 'earliest';

      assert.equal(format.format('Q|T', pendingTag, false), pendingTag);
      assert.equal(format.format('Q|T', latestTag, false), latestTag);
      assert.equal(format.format('Q|T', earliestTag, false), earliestTag);

      const r1 = format.format('Q|T', prefixHexNumber, false).toString(10);
      const r2 = format.format('Q|T', noPrefixHexNumber, false).toString(10);
      const r3 = format.format('Q|T', largeHexNumber, false).toString(10);
      const r4 = format.format('Q|T', bignum, false).toString(10);
      const r5 = format.format('Q|T', num, false).toString(10);
      const r6 = format.format('Q|T', stringNumber, false).toString(10);
      const r7 = format.format('Q|T', nullNumber, false);

      assert.equal(r1, '23492348972');
      assert.equal(r2, '35463586');
      assert.equal(r3, '9999999999999999');
      assert.equal(r4, '45333');
      assert.equal(r5, '327243762');
      assert.equal(r6, '238428237842');
      assert.equal(r7, null);
      assert.equal(format.format('Q|T', undefinedNumber, false), undefinedNumber);
    });

    it('should decode object SendTransaction object normally', () => {
      const encodedSendTransactionObject = {
        'from': '0xb60e8dd61c5d32be8058bb8eb970870f07233155',
        'to': '0xd46e8dd67c5d32be8058bb8eb970870f07244567',
        'gas': '', // 30400,
        'gasPrice': '0x9184e72a000', // 10000000000000
        'value': '0x9184e72a', // 2441406250
        'data': ''
      };

      const decodedObject = format.format('SendTransaction', encodedSendTransactionObject, false);

      assert.equal(decodedObject.from, '0xb60e8dd61c5d32be8058bb8eb970870f07233155');
      assert.equal(decodedObject.to, '0xd46e8dd67c5d32be8058bb8eb970870f07244567');
      assert.equal(decodedObject.data, '0x');

      assert.equal(decodedObject.gas.toString(10), '0');
      assert.equal(decodedObject.gasPrice.toString(10), '10000000000000');
      assert.equal(decodedObject.value.toString(10), '2441406250');
    });

    it('should encode object SendTransaction object normally', () => {
      const decodedSendTransactionObject_1 = {
        'from': '0xb60e8dd61c5d32be8058bb8eb970870f07233155',
        'to': '0xd46e8dd67c5d32be8058bb8eb970870f07244567',
        'gas': new BN('30400'), // 30400,
        'gasPrice': '10000000000000', // 10000000000000
        'value': 2441406250, // 2441406250
        'data': '0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675'
      };

      const encodedObject_1 = format.format('SendTransaction', decodedSendTransactionObject_1, true);

      assert.equal(encodedObject_1.from, '0xb60e8dd61c5d32be8058bb8eb970870f07233155');
      assert.equal(encodedObject_1.to, '0xd46e8dd67c5d32be8058bb8eb970870f07244567');
      assert.equal(encodedObject_1.data, '0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675');

      assert.equal(encodedObject_1.gas, '0x76c0');
      assert.equal(encodedObject_1.gasPrice, '0x9184e72a000');
      assert.equal(encodedObject_1.value, '0x9184e72a');
    });

    it ('should encode null normally', () => {
      const encodeNull1 = format.format('SendTransaction', null, true);
      const encodeNull2 = format.format('Filter', null, true);
      const encodeNull3 = format.format('D', null, true);
      const encodeNull4 = format.format('B', null, true);
      const encodeNull5 = format.format('S', null, true);
      const encodeNull6 = format.format('Q', null, true);
      const encodeNull7 = format.format('Q|T', null, true);
      const encodeNull8 = format.format('Block', null, true);
      const encodeNull9 = format.format('Array|DATA', null, true);
      const encodeNull10 = format.format(['D'], null, true);

      assert.equal(encodeNull1, null);
      assert.equal(encodeNull2, null);
      assert.equal(encodeNull3, null);
      assert.equal(encodeNull4, null);
      assert.equal(encodeNull5, null);
      assert.equal(encodeNull6, null);
      assert.equal(encodeNull7, null);
      assert.equal(encodeNull8, null);
      assert.equal(encodeNull9, null);
      assert.equal(encodeNull10, null);
    });

    it ('should handle 20 and 32 byte data properly', () => {
      assert.equal(format.format('D32', '0x9646252be9520f6e71339a8df9c55e4d7619deeb018d2a3f2d21fc165dde5eb5', false), '0x9646252be9520f6e71339a8df9c55e4d7619deeb018d2a3f2d21fc165dde5eb5');
      assert.equal(format.format('D32', '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d32', true), '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d32');
      assert.equal(format.format('D20', '0x407d73d8a49eeb85d32cf465507dd71d507100c1', true), '0x407d73d8a49eeb85d32cf465507dd71d507100c1');
      assert.equal(format.format('D20', '0x85e43d8a49eeb85d32cf465507dd71d507100c12', false), '0x85e43d8a49eeb85d32cf465507dd71d507100c12');
      assert.equal(format.format('D20', '0x', false), '0x');
      var invalidBytesError;

      try {
        format.format('D20', '*((*(dsfjj)))', false);
      } catch (errNonAlpha) {
        assert.equal(typeof errNonAlpha, 'object');
      }

      try {
        format.format('D32', '0x0', true);
        format.format('D32', '0x89iusdf', false);
        format.format('D20', '0x038372', true);
        format.format('D20', '0x038372', false);
      } catch (error) {
        invalidBytesError = error;
      }

      assert.equal(typeof invalidBytesError, 'object');
    });

    it ('should decode null normally', () => {
      const decodedNull1 = format.format('SendTransaction', null, false);
      const decodedNull2 = format.format('Filter', null, false);
      const decodedNull3 = format.format('D', null, false);
      const decodedNull4 = format.format('B', null, false);
      const decodedNull5 = format.format('S', null, false);
      const decodedNull6 = format.format('Q', null, false);
      const decodedNull7 = format.format('Q|T', null, false);
      const decodedNull8 = format.format('Block', null, false);
      const decodedNull9 = format.format('Array|DATA', null, false);
      const decodedNull10 = format.format(['D'], null, false);

      assert.equal(decodedNull1, null);
      assert.equal(decodedNull2, null);
      assert.equal(decodedNull3, null);
      assert.equal(decodedNull4, null);
      assert.equal(decodedNull5, null);
      assert.equal(decodedNull6, null);
      assert.equal(decodedNull7, null);
      assert.equal(decodedNull8, null);
      assert.equal(decodedNull9, null);
      assert.equal(decodedNull10, null);
    });

    it ('should handle arrays normally normally', () => {
      assert.equal(format.format(['D'], [''], false)[0], ['0x'][0]);
      assert.equal(format.format(['D'], [''], true)[0], ['0x'][0]);
      assert.equal(format.format(['D'], ['0x'], false)[0], ['0x'][0]);
      assert.equal(format.format(['D'], ['0x'], true)[0], ['0x'][0]);
      assert.equal(format.format(['B'], [true], false)[0], [true][0]);
      assert.equal(format.format(['B'], [false], true)[0], [false][0]);
      assert.equal(format.format('Array|DATA', '0x', true), '0x');
      assert.equal(format.format('Array|DATA', '0x', false), '0x');
    });
  });

  describe('test formatInputs', () => {
    it ('should handle eth_getBalance normally', () => {
      const encodedBalance = format.formatInputs('eth_getBalance', ['0xb60e8dd61c5d32be8058bb8eb970870f07233155']);

      assert.equal(encodedBalance, '0xb60e8dd61c5d32be8058bb8eb970870f07233155');
    });

    it ('should handle eth_getBalance 2 arguments normally', () => {
      const encodedBalance = format.formatInputs('eth_getBalance', ['0xb60e8dd61c5d32be8058bb8eb970870f07233155', 'latest']);

      assert.equal(encodedBalance[0], '0xb60e8dd61c5d32be8058bb8eb970870f07233155');
      assert.equal(encodedBalance[1], 'latest');
    });

    it ('should handle eth_sendTransaction normally', () => {
      const encodedSendTransaction = format.formatInputs('eth_sendTransaction', [{
        'from': '0xb60e8dd61c5d32be8058bb8eb970870f07233155',
        'to': '0xd46e8dd67c5d32be8058bb8eb970870f07244567',
        'gas': new BN('30400'), // 30400,
        'gasPrice': '10000000000000', // 10000000000000
        'value': 2441406250, // 2441406250
        'data': '0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675'
      }]);
      const encodedSendTransactionProper = [{
        from: '0xb60e8dd61c5d32be8058bb8eb970870f07233155',
        to: '0xd46e8dd67c5d32be8058bb8eb970870f07244567',
        gas: '0x76c0',
        gasPrice: '0x9184e72a000',
        value: '0x9184e72a',
        data: '0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675'
      }];

      assert.equal(encodedSendTransaction[0].from, encodedSendTransactionProper[0].from);
      assert.equal(encodedSendTransaction[0].to, encodedSendTransactionProper[0].to);
      assert.equal(encodedSendTransaction[0].gas, encodedSendTransactionProper[0].gas);
      assert.equal(encodedSendTransaction[0].gasPrice, encodedSendTransactionProper[0].gasPrice);
      assert.equal(encodedSendTransaction[0].value, encodedSendTransactionProper[0].value);
      assert.equal(encodedSendTransaction[0].data, encodedSendTransactionProper[0].data);
    });
  });

  describe('test formatOutputs', () => {
    it ('should handle invalid eth_getTransactionByBlockHashAndIndex normally', () => {
      try {
        const decodedHashRate = format.formatInputs('eth_getTransactionByBlockHashAndIndex', ['0x']);
      } catch (error) {
        assert.equal(typeof error, 'object');
      }
    });

    it ('should handle eth_getFilterChanges normally during eth_newBlockFilter', () => {
      const decodedTxHashArray = format.formatOutputs('eth_getFilterChanges', ['0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55', '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55']);

      assert.equal(decodedTxHashArray.toString(10), ['0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55', '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55']);
    });

    it ('should handle eth_getFilterChanges normally during eth_newPendingTransactionFilter', () => {
      const decodedTxHashArray = format.formatOutputs('eth_getFilterChanges', ['0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55', '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55']);

      assert.equal(decodedTxHashArray.toString(10), ['0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55', '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55']);
    });

    it ('should handle eth_hashrate normally', () => {
      const decodedHashRate = format.formatOutputs('eth_hashrate', '0x38a');

      assert.equal(decodedHashRate.toString(10), '906');
    });

    it ('should handle eth_getBalance normally', () => {
      const decodedBalance = format.formatOutputs('eth_getBalance', '0x38a');

      assert.equal(decodedBalance.toString(10), '906');
    });

    it ('should handle empty eth_hashrate normally', () => {
      const decodedHashRate = format.formatOutputs('eth_hashrate', '0x');

      assert.equal(decodedHashRate.toString(10), '0');
    });

    it ('should handle eth_mining true normally', () => {
      const decodedMining = format.formatOutputs('eth_mining', true);

      assert.equal(decodedMining, true);
    });

    it ('should handle eth_mining false normally', () => {
      const decodedMining = format.formatOutputs('eth_mining', false);

      assert.equal(decodedMining, false);
    });

    it ('should handle eth_accounts normally', () => {
      const decodedEthAccounts = format.formatOutputs('eth_accounts', ['0x407d73d8a49eeb85d32cf465507dd71d507100c1', '0x407d73d8a49eeb85d32cf465507dd71d507100c1', '0x407d73d8a49eeb85d32cf465507dd71d507100c1']);

      assert.equal(decodedEthAccounts[0], '0x407d73d8a49eeb85d32cf465507dd71d507100c1');
      assert.equal(decodedEthAccounts[1], '0x407d73d8a49eeb85d32cf465507dd71d507100c1');
      assert.equal(decodedEthAccounts[2], '0x407d73d8a49eeb85d32cf465507dd71d507100c1');
    });

    it ('should handle empty eth_accounts normally', () => {
      const decodedEthAccounts = format.formatOutputs('eth_accounts', []);

      assert.equal(decodedEthAccounts.length, 0);
    });

    it ('should handle eth_getFilterChanges normally', () => {
      const decodedFilterChanges = format.formatOutputs('eth_getFilterChanges', [{
        'logIndex': '0x1', // 1
        'blockNumber':'0x1b4', // 436
        'blockHash': '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d21',
        'transactionHash':  '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55',
        'transactionIndex': '0x0', // 0
        'address': '0x16c5785ac562ff41e2dcfdf829c5a142f1fccd7d',
        'data':'0x0000000000000000000000000000000000000000000000000000000000000000',
        'topics': ['0x59ebeb90bc63057b6515673c3ecf9438e5058bca0f92585014eced636878c9a5']
      }, {
        'logIndex': '0x1', // 1
        'blockNumber':'0x1b4', // 436
        'blockHash': '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d21',
        'transactionHash':  '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55',
        'transactionIndex': '0x0', // 0
        'address': '0x16c5785ac562ff41e2dcfdf829c5a142f1fccd7d',
        'data':'0x0000000000000000000000000000000000000000000000000000000000000000',
        'topics': ['0x59ebeb90bc63057b6515673c3ecf9438e5058bca0f92585014eced636878c9a5']
      }]);

      assert.equal(decodedFilterChanges[0].logIndex.toString(10), '1');
      assert.equal(decodedFilterChanges[0].blockNumber.toString(10), '436');
      assert.equal(decodedFilterChanges[0].transactionIndex.toString(10), '0');
      assert.equal(decodedFilterChanges[0].address, '0x16c5785ac562ff41e2dcfdf829c5a142f1fccd7d');
      assert.equal(decodedFilterChanges[0].data, '0x0000000000000000000000000000000000000000000000000000000000000000');
      assert.equal(decodedFilterChanges[0].blockHash, '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d21');
      assert.equal(decodedFilterChanges[0].transactionHash, '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55');
      assert.equal(decodedFilterChanges[0].topics[0], '0x59ebeb90bc63057b6515673c3ecf9438e5058bca0f92585014eced636878c9a5');

      assert.equal(decodedFilterChanges[1].logIndex.toString(10), '1');
      assert.equal(decodedFilterChanges[1].blockNumber.toString(10), '436');
      assert.equal(decodedFilterChanges[1].transactionIndex.toString(10), '0');
      assert.equal(decodedFilterChanges[1].address, '0x16c5785ac562ff41e2dcfdf829c5a142f1fccd7d');
      assert.equal(decodedFilterChanges[1].data, '0x0000000000000000000000000000000000000000000000000000000000000000');
      assert.equal(decodedFilterChanges[1].blockHash, '0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d21');
      assert.equal(decodedFilterChanges[1].transactionHash, '0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf55');
      assert.equal(decodedFilterChanges[1].topics[0], '0x59ebeb90bc63057b6515673c3ecf9438e5058bca0f92585014eced636878c9a5');
    });
  });
});

/*
'EthSyncing',
'SendTransaction',
'Block',
'Transaction',
'Receipt',
'Filter',
'FilterChange',
'SHHPost',
'SHHFilter',
'SHHMessage',

'DATA|Transaction'
'Boolean|EthSyncing'

'Array|DATA'

'Q'
'Q|T'

'B'
'S'
'D'
*/
