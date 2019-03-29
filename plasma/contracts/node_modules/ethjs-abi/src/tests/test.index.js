const assert = require('chai').assert;
const abi = require('../index.js');
const contracts = require('./contracts.json');
const BN = require('bn.js');

describe('test basic encoding and decoding functionality', () => {
  const interfaceABI = [
  {'constant':false,'inputs':[{'name':'_value','type':'uint256'}],'name':'set','outputs':[{'name':'','type':'bool'}],'payable':false,'type':'function'}, {'constant':false,'inputs':[],'name':'get','outputs':[{'name':'storeValue','type':'uint256'}],'payable':false,'type':'function'}, {'anonymous':false,'inputs':[{'indexed':false,'name':'_newValue','type':'uint256'},{'indexed':false,'name':'_sender','type':'address'}],'name':'SetComplete','type':'event'}]; // eslint-disable-line
  it('should encode and decode contract data nicely', () => {
    const BalanceClaimInterface = JSON.parse(contracts.BalanceClaim.interface);
    const encodeBalanceClaimMethod1 = abi.encodeMethod(BalanceClaimInterface[0], []);
    assert.equal(encodeBalanceClaimMethod1, '0x30509bca');

    const setMethodInputBytecode = abi.encodeMethod(interfaceABI[0], [24000]);
    abi.decodeMethod(interfaceABI[0], '0x0000000000000000000000000000000000000000000000000000000000000001');

    abi.encodeMethod(interfaceABI[1], []);
    abi.decodeMethod(interfaceABI[1], '0x000000000000000000000000000000000000000000000000000000000000b26e');

    abi.encodeEvent(interfaceABI[2], [24000, '0xca35b7d915458ef540ade6068dfe2f44e8fa733c']);
    const event = abi.decodeEvent(interfaceABI[2], '0x0000000000000000000000000000000000000000000000000000000000000d7d000000000000000000000000ca35b7d915458ef540ade6068dfe2f44e8fa733c');
    assert.deepEqual(event, {
      0: new BN(3453),
      1: '0xca35b7d915458ef540ade6068dfe2f44e8fa733c',
      _eventName: 'SetComplete',
      _newValue: new BN(3453),
      _sender: '0xca35b7d915458ef540ade6068dfe2f44e8fa733c',
    });
    assert.equal(setMethodInputBytecode, '0x60fe47b10000000000000000000000000000000000000000000000000000000000005dc0');
  });

  it('should decode event from log', () => {
    const eventAbi = {
      anonymous: false,
      inputs: [
        { indexed: true, name: 'userKey', type: 'address' },
        { indexed: false, name: 'proxy', type: 'address' },
        { indexed: false, name: 'controller', type: 'address' },
        { indexed: false, name: 'recoveryKey', type: 'address' },
      ],
      name: 'IdentityCreated',
      type: 'event',
    };
    assert.equal(abi.eventSignature(eventAbi), '0xc36800ebd6079fdafc3a7100d0d1172815751804a6d1b7eb365b85f6c9c80e61');
    const logs = [{
      address: '0xadb4966858672ef5ed70894030526544f9a5acdd',
      topics: [
        '0xc36800ebd6079fdafc3a7100d0d1172815751804a6d1b7eb365b85f6c9c80e61',
        '0x00000000000000000000000050858f2c7873fac9398ed9c195d185089caa7967',
      ],
      data: '0x0000000000000000000000000aa622ec7d114c8a18730a9a6147ffbded11cefa000000000000000000000000cec030978d9e5e8b4ad689b1f509f8e9617efbe300000000000000000000000041f50a40900dc9ac8a6d4cfb4fa5e05ed428de42',
      blockNumber: 516657,
      transactionIndex: 1,
      transactionHash: '0x94761202b5fdcf50dfa8cc07abcc10b58744b985470dce6ab97b315a03f7f185',
      blockHash: '0x0c8092332fea2f708bcc6e4c105b7a5dc2bdf920e36e7336749c62e307be04e9',
      logIndex: 0,
    }];

    const decoded = abi.decodeEvent(eventAbi, logs[0].data, logs[0].topics);
    assert.deepEqual(decoded, {
      0: '0x0aa622ec7d114c8a18730a9a6147ffbded11cefa',
      1: '0xcec030978d9e5e8b4ad689b1f509f8e9617efbe3',
      2: '0x41f50a40900dc9ac8a6d4cfb4fa5e05ed428de42',
      _eventName: 'IdentityCreated',
      userKey: '0x50858f2c7873fac9398ed9c195d185089caa7967',
      proxy: '0x0aa622ec7d114c8a18730a9a6147ffbded11cefa',
      controller: '0xcec030978d9e5e8b4ad689b1f509f8e9617efbe3',
      recoveryKey: '0x41f50a40900dc9ac8a6d4cfb4fa5e05ed428de42',
    });

    assert.deepEqual(abi.decodeLogItem(eventAbi, logs[0], false), {
      _eventName: 'IdentityCreated',
      userKey: '0x50858f2c7873fac9398ed9c195d185089caa7967',
      proxy: '0x0aa622ec7d114c8a18730a9a6147ffbded11cefa',
      controller: '0xcec030978d9e5e8b4ad689b1f509f8e9617efbe3',
      recoveryKey: '0x41f50a40900dc9ac8a6d4cfb4fa5e05ed428de42',
    });

    const decode = abi.logDecoder([eventAbi].concat(interfaceABI), false);

    assert.deepEqual(decode(logs), [{
      _eventName: 'IdentityCreated',
      userKey: '0x50858f2c7873fac9398ed9c195d185089caa7967',
      proxy: '0x0aa622ec7d114c8a18730a9a6147ffbded11cefa',
      controller: '0xcec030978d9e5e8b4ad689b1f509f8e9617efbe3',
      recoveryKey: '0x41f50a40900dc9ac8a6d4cfb4fa5e05ed428de42',
    }]);
  });
});
