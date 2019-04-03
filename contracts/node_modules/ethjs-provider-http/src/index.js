/**
 * @original-authors:
 *   Marek Kotewicz <marek@ethdev.com>
 *   Marian Oancea <marian@ethdev.com>
 *   Fabian Vogelsteller <fabian@ethdev.com>
 * @date 2015
 */

// workaround to use httpprovider in different envs
const XHR2 = require('xhr2');

/*
""
responseText
:
""
responseType
:
""
responseURL
:
"https://ropsten.infura.io/"
responseXML
:
null
status
:
405
statusText
:
"Method Not Allowed"
timeout
:
0
*/

/**
 * InvalidResponseError helper for invalid errors.
 */
function invalidResponseError(request, host) {
  const responseError = new Error(`[ethjs-provider-http] Invalid JSON RPC response from provider
    host: ${host}
    response: ${String(request.responseText)} ${JSON.stringify(request.responseText, null, 2)}
    responseURL: ${request.responseURL}
    status: ${request.status}
    statusText: ${request.statusText}
  `);
  responseError.value = request;
  return responseError;
}

/**
 * HttpProvider should be used to send rpc calls over http
 */
function HttpProvider(host, timeout) {
  if (!(this instanceof HttpProvider)) { throw new Error('[ethjs-provider-http] the HttpProvider instance requires the "new" flag in order to function normally (e.g. `const eth = new Eth(new HttpProvider());`).'); }
  if (typeof host !== 'string') { throw new Error('[ethjs-provider-http] the HttpProvider instance requires that the host be specified (e.g. `new HttpProvider("http://localhost:8545")` or via service like infura `new HttpProvider("http://ropsten.infura.io")`)'); }

  const self = this;
  self.host = host;
  self.timeout = timeout || 0;
}

/**
 * Should be used to make async request
 *
 * @method sendAsync
 * @param {Object} payload
 * @param {Function} callback triggered on end with (err, result)
 */
HttpProvider.prototype.sendAsync = function (payload, callback) { // eslint-disable-line
  const self = this;
  var request = new XHR2(); // eslint-disable-line

  request.timeout = self.timeout;
  request.open('POST', self.host, true);
  request.setRequestHeader('Content-Type', 'application/json');

  request.onreadystatechange = () => {
    if (request.readyState === 4 && request.timeout !== 1) {
      var result = request.responseText; // eslint-disable-line
      var error = null; // eslint-disable-line

      try {
        result = JSON.parse(result);
      } catch (jsonError) {
        error = invalidResponseError(request, self.host);
      }

      callback(error, result);
    }
  };

  request.ontimeout = () => {
    callback(`[ethjs-provider-http] CONNECTION TIMEOUT: http request timeout after ${self.timeout} ms. (i.e. your connect has timed out for whatever reason, check your provider).`, null);
  };

  try {
    request.send(JSON.stringify(payload));
  } catch (error) {
    callback(`[ethjs-provider-http] CONNECTION ERROR: Couldn't connect to node '${self.host}': ${JSON.stringify(error, null, 2)}`, null);
  }
};

module.exports = HttpProvider;
