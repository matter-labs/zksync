const format = require('ethjs-format');
const EthRPC = require('ethjs-rpc');
const promiseToCallback = require('promise-to-callback');

module.exports = Eth;

function Eth(provider, options) {
  const self = this;
  const optionsObject = options || {};

  if (!(this instanceof Eth)) { throw new Error('[ethjs-query] the Eth object requires the "new" flag in order to function normally (i.e. `const eth = new Eth(provider);`).'); }
  if (typeof provider !== 'object') { throw new Error(`[ethjs-query] the Eth object requires that the first input 'provider' must be an object, got '${typeof provider}' (i.e. 'const eth = new Eth(provider);')`); }

  self.options = Object.assign({
    debug: optionsObject.debug || false,
    logger: optionsObject.logger || console,
    jsonSpace: optionsObject.jsonSpace || 0,
  });
  self.rpc = new EthRPC(provider);
  self.setProvider = self.rpc.setProvider;
}

Eth.prototype.log = function log(message) {
  const self = this;
  if (self.options.debug) self.options.logger.log(`[ethjs-query log] ${message}`);
};

Object.keys(format.schema.methods).forEach((rpcMethodName) => {
  Object.defineProperty(Eth.prototype, rpcMethodName.replace('eth_', ''), {
    enumerable: true,
    value: generateFnFor(rpcMethodName, format.schema.methods[rpcMethodName]),
  });
});

function generateFnFor(rpcMethodName, methodObject) {
  return function outputMethod() {
    let callback = null; // eslint-disable-line
    let inputs = null; // eslint-disable-line
    let inputError = null; // eslint-disable-line
    const self = this;
    const args = [].slice.call(arguments); // eslint-disable-line
    const protoMethodName = rpcMethodName.replace('eth_', ''); // eslint-disable-line

    if (args.length > 0 && typeof args[args.length - 1] === 'function') {
      callback = args.pop();
    }

    const promise = performCall.call(this);

    // if callback provided, convert promise to callback
    if (callback) {
      return promiseToCallback(promise)(callback);
    }

    // only return promise if no callback provided
    return promise;

    function performCall() {
      return new Promise((resolve, reject) => {
        // validate arg length
        if (args.length < methodObject[2]) {
          reject(new Error(`[ethjs-query] method '${protoMethodName}' requires at least ${methodObject[2]} input (format type ${methodObject[0][0]}), ${args.length} provided. For more information visit: https://github.com/ethereum/wiki/wiki/JSON-RPC#${rpcMethodName.toLowerCase()}`));
          return;
        }
        if (args.length > methodObject[0].length) {
          reject(new Error(`[ethjs-query] method '${protoMethodName}' requires at most ${methodObject[0].length} params, ${args.length} provided '${JSON.stringify(args, null, self.options.jsonSpace)}'. For more information visit: https://github.com/ethereum/wiki/wiki/JSON-RPC#${rpcMethodName.toLowerCase()}`));
          return;
        }

        // set default block
        if (methodObject[3] && args.length < methodObject[3]) {
          args.push('latest');
        }

        // format inputs
        this.log(`attempting method formatting for '${protoMethodName}' with inputs ${JSON.stringify(args, null, this.options.jsonSpace)}`);
        try {
          inputs = format.formatInputs(rpcMethodName, args);
          this.log(`method formatting success for '${protoMethodName}' with formatted result: ${JSON.stringify(inputs, null, this.options.jsonSpace)}`);
        } catch (formattingError) {
          reject(new Error(`[ethjs-query] while formatting inputs '${JSON.stringify(args, null, this.options.jsonSpace)}' for method '${protoMethodName}' error: ${formattingError}`));
          return;
        }

        // perform rpc call
        this.rpc.sendAsync({ method: rpcMethodName, params: inputs })
        .then(result => {
          // format result
          try {
            this.log(`attempting method formatting for '${protoMethodName}' with raw outputs: ${JSON.stringify(result, null, this.options.jsonSpace)}`);
            const methodOutputs = format.formatOutputs(rpcMethodName, result);
            this.log(`method formatting success for '${protoMethodName}' formatted result: ${JSON.stringify(methodOutputs, null, this.options.jsonSpace)}`);
            resolve(methodOutputs);
            return;
          } catch (outputFormattingError) {
            const outputError = new Error(`[ethjs-query] while formatting outputs from RPC '${JSON.stringify(result, null, this.options.jsonSpace)}' for method '${protoMethodName}' ${outputFormattingError}`);
            reject(outputError);
            return;
          }
        })
        .catch(error => {
          const outputError = new Error(`[ethjs-query] while formatting outputs from RPC '${JSON.stringify(error, null, this.options.jsonSpace)}'`);
          reject(outputError);
          return;
        });
      });
    }
  };
}
