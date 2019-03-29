const EthQuery = require('ethjs-query');
const EthFilter = require('ethjs-filter');
const EthContract = require('ethjs-contract');
const HttpProvider = require('ethjs-provider-http');
const abi = require('ethjs-abi');
// const getTxSuccess = require('ethjs-transaction-success'); // eslint-disable-line
const unit = require('ethjs-unit');
const keccak256 = require('js-sha3').keccak_256;
const toBN = require('number-to-bn');
const BN = require('bn.js');
const utils = require('ethjs-util');
const getTransactionSuccess = require('./lib/getTransactionSuccess.js');

module.exports = Eth;

/**
 * Returns the ethjs Eth instance.
 *
 * @method Eth
 * @param {Object} cprovider the web3 standard provider object
 * @param {Object} options the Eth options object
 * @returns {Object} eth Eth object instance
 * @throws if the new flag is not used in construction
 */

function Eth(cprovider, options) {
  if (!(this instanceof Eth)) { throw new Error('[ethjs] the Eth object requires you construct it with the "new" flag (i.e. `const eth = new Eth(...);`).'); }
  const self = this;
  self.options = options || {};
  const query = new EthQuery(cprovider, self.options.query);
  Object.keys(Object.getPrototypeOf(query)).forEach((methodName) => {
    self[methodName] = (...args) => query[methodName].apply(query, args);
  });
  self.filter = new EthFilter(query, self.options.query);
  self.contract = new EthContract(query, self.options.query);
  self.currentProvider = query.rpc.currentProvider;
  self.setProvider = query.setProvider;
  self.getTransactionSuccess = getTransactionSuccess(self);
}

Eth.BN = BN;
Eth.isAddress = (val) => utils.isHexString(val, 20);
Eth.keccak256 = (val) => `0x${keccak256(val)}`;
Eth.Buffer = Buffer;
Eth.isHexString = utils.isHexString;
Eth.fromWei = unit.fromWei;
Eth.toWei = unit.toWei;
Eth.toBN = toBN;
Eth.abi = abi;
Eth.fromAscii = utils.fromAscii;
Eth.toAscii = utils.toAscii;
Eth.fromUtf8 = utils.fromUtf8;
Eth.toUtf8 = utils.toUtf8;
Eth.HttpProvider = HttpProvider;
