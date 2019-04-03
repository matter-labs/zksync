const abi = require('ethjs-abi'); // eslint-disable-line
const EthFilter = require('ethjs-filter'); // eslint-disable-line
const getKeys = require('ethjs-util').getKeys; // eslint-disable-line
const keccak256 = require('js-sha3').keccak_256; // eslint-disable-line
const hasTransactionObject = require('./has-tx-object');
const promiseToCallback = require('promise-to-callback');

module.exports = Contract;

function Contract(opts = {}) {
  const self = this;
  self.abi = opts.contractABI || [];
  self.query = opts.query;
  self.address = opts.address || '0x';
  self.bytecode = opts.contractBytecode || '0x';
  self.defaultTxObject = opts.contractDefaultTxObject || {};
  self.filters = new EthFilter(self.query);

  getCallableMethodsFromABI(self.abi).forEach((methodObject) => {
    if (methodObject.type === 'function') {
      self[methodObject.name] = createContractFunction(methodObject);
    } else if (methodObject.type === 'event') {
      self[methodObject.name] = createContractEvent(methodObject);
    }
  });

  function createContractEvent(methodObject) {
    return function contractEvent() {
      const methodArgs = [].slice.call(arguments); // eslint-disable-line

      const filterInputTypes = getKeys(methodObject.inputs, 'type', false);
      const filterTopic = `0x${keccak256(`${methodObject.name}(${filterInputTypes.join(',')})`)}`;
      const filterTopcis = [filterTopic];
      const argsObject = Object.assign({}, methodArgs[0]) || {};

      const defaultFilterObject = Object.assign({}, (methodArgs[0] || {}), {
        to: self.address,
        topics: filterTopcis,
      });
      const filterOpts = Object.assign({}, argsObject, {
        decoder: (logData) => abi.decodeEvent(methodObject, logData, filterTopcis),
        defaultFilterObject,
      });

      return new self.filters.Filter(filterOpts);
    };
  }

  function createContractFunction(methodObject) {
    return function contractFunction() {
      let methodCallback; // eslint-disable-line
      const methodArgs = [].slice.call(arguments); // eslint-disable-line
      if (typeof methodArgs[methodArgs.length - 1] === 'function') {
        methodCallback = methodArgs.pop();
      }

      const promise = performCall({ methodObject, methodArgs });

      if (methodCallback) {
        return promiseToCallback(promise)(methodCallback);
      }

      return promise;
    };
  }

  async function performCall({ methodObject, methodArgs }) {
    let queryMethod = 'call'; // eslint-disable-line
    let providedTxObject = {}; // eslint-disable-line

    if (hasTransactionObject(methodArgs)) providedTxObject = methodArgs.pop();
    const methodTxObject = Object.assign({},
      self.defaultTxObject,
      providedTxObject, {
        to: self.address,
      });
    methodTxObject.data = abi.encodeMethod(methodObject, methodArgs);

    if (methodObject.constant === false) {
      queryMethod = 'sendTransaction';
    }

    const queryResult = await self.query[queryMethod](methodTxObject);

    if (queryMethod === 'call') {
      // queryMethod is 'call', result is returned value
      try {
        const decodedMethodResult = abi.decodeMethod(methodObject, queryResult);
        return decodedMethodResult;
      } catch (decodeFormattingError) {
        const decodingError = new Error(`[ethjs-contract] while formatting incoming raw call data ${JSON.stringify(queryResult)} ${decodeFormattingError}`);
        throw decodingError;
      }
    }
    // queryMethod is 'sendTransaction', result is txHash
    return queryResult;
  }
}

function getCallableMethodsFromABI(contractABI) {
  return contractABI.filter((json) => ((json.type === 'function' || json.type === 'event') && json.name.length > 0));
}
