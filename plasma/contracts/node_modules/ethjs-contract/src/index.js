const abi = require('ethjs-abi'); // eslint-disable-line
const keccak256 = require('js-sha3').keccak_256; // eslint-disable-line
const EthFilter = require('ethjs-filter'); // eslint-disable-line
const getKeys = require('ethjs-util').getKeys; // eslint-disable-line
const Contract = require('./contract');
const hasTransactionObject = require('./has-tx-object');

module.exports = EthContract;

function EthContract(query) {
  return function contractFactory(contractABI, contractBytecode, contractDefaultTxObject) {
    // validate params
    if (!Array.isArray(contractABI)) { throw new Error(`[ethjs-contract] Contract ABI must be type Array, got type ${typeof contractABI}`); }
    if (typeof contractBytecode !== 'undefined' && typeof contractBytecode !== 'string') { throw new Error(`[ethjs-contract] Contract bytecode must be type String, got type ${typeof contractBytecode}`); }
    if (typeof contractDefaultTxObject !== 'undefined' && typeof contractDefaultTxObject !== 'object') { throw new Error(`[ethjs-contract] Contract default tx object must be type Object, got type ${typeof contractABI}`); }

    // build contract object
    const output = {};
    output.at = function contractAtAddress(address) {
      return new Contract({
        address,
        query,
        contractBytecode,
        contractDefaultTxObject,
        contractABI,
      });
    };

    output.new = function newContract() {
      let providedTxObject = {}; // eslint-disable-line
      let newMethodCallback = null; // eslint-disable-line
      const newMethodArgs = [].slice.call(arguments); // eslint-disable-line
      if (typeof newMethodArgs[newMethodArgs.length - 1] === 'function') newMethodCallback = newMethodArgs.pop();
      if (hasTransactionObject(newMethodArgs)) providedTxObject = newMethodArgs.pop();
      const constructorMethod = getConstructorFromABI(contractABI);
      const assembleTxObject = Object.assign({}, contractDefaultTxObject, providedTxObject);

      // set contract deploy bytecode
      if (contractBytecode) {
        assembleTxObject.data = contractBytecode;
      }

      // append encoded constructor arguments
      if (constructorMethod) {
        const constructorBytecode = abi.encodeParams(getKeys(constructorMethod.inputs, 'type'), newMethodArgs).substring(2); // eslint-disable-line
        assembleTxObject.data = `${assembleTxObject.data}${constructorBytecode}`;
      }

      return newMethodCallback ? query.sendTransaction(assembleTxObject, newMethodCallback) : query.sendTransaction(assembleTxObject);
    };

    return output;
  };
}

function getConstructorFromABI(contractABI) {
  return contractABI.filter((json) => (json.type === 'constructor'))[0];
}
