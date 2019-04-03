 /* eslint-disable */ 
(function webpackUniversalModuleDefinition(root, factory) {
	if(typeof exports === 'object' && typeof module === 'object')
		module.exports = factory();
	else if(typeof define === 'function' && define.amd)
		define("EthFilter", [], factory);
	else if(typeof exports === 'object')
		exports["EthFilter"] = factory();
	else
		root["EthFilter"] = factory();
})(this, function() {
return /******/ (function(modules) { // webpackBootstrap
/******/ 	// The module cache
/******/ 	var installedModules = {};
/******/
/******/ 	// The require function
/******/ 	function __webpack_require__(moduleId) {
/******/
/******/ 		// Check if module is in cache
/******/ 		if(installedModules[moduleId])
/******/ 			return installedModules[moduleId].exports;
/******/
/******/ 		// Create a new module (and put it into the cache)
/******/ 		var module = installedModules[moduleId] = {
/******/ 			i: moduleId,
/******/ 			l: false,
/******/ 			exports: {}
/******/ 		};
/******/
/******/ 		// Execute the module function
/******/ 		modules[moduleId].call(module.exports, module, module.exports, __webpack_require__);
/******/
/******/ 		// Flag the module as loaded
/******/ 		module.l = true;
/******/
/******/ 		// Return the exports of the module
/******/ 		return module.exports;
/******/ 	}
/******/
/******/
/******/ 	// expose the modules object (__webpack_modules__)
/******/ 	__webpack_require__.m = modules;
/******/
/******/ 	// expose the module cache
/******/ 	__webpack_require__.c = installedModules;
/******/
/******/ 	// identity function for calling harmory imports with the correct context
/******/ 	__webpack_require__.i = function(value) { return value; };
/******/
/******/ 	// define getter function for harmory exports
/******/ 	__webpack_require__.d = function(exports, name, getter) {
/******/ 		Object.defineProperty(exports, name, {
/******/ 			configurable: false,
/******/ 			enumerable: true,
/******/ 			get: getter
/******/ 		});
/******/ 	};
/******/
/******/ 	// Object.prototype.hasOwnProperty.call
/******/ 	__webpack_require__.o = function(object, property) { return Object.prototype.hasOwnProperty.call(object, property); };
/******/
/******/ 	// __webpack_public_path__
/******/ 	__webpack_require__.p = "";
/******/
/******/ 	// Load entry module and return exports
/******/ 	return __webpack_require__(__webpack_require__.s = 0);
/******/ })
/************************************************************************/
/******/ ([
/* 0 */
/***/ function(module, exports) {

"use strict";
'use strict';

function constructFilter(filterName, query) {
  function Filter(options) {
    var self = this;
    self.filterId = null;
    self.options = Object.assign({
      delay: 300,
      decoder: function decodeData(data) {
        return data;
      },
      defaultFilterObject: {}
    }, options || {});

    self.watchers = {};
    self.interval = setInterval(function () {
      if (self.filterId !== null && Object.keys(self.watchers).length > 0) {
        query.getFilterChanges(self.filterId, function (changeError, changeResult) {
          var decodedChangeResults = [];
          var decodingError = null; // eslint-disable-line

          if (!changeError) {
            try {
              changeResult.forEach(function (log, logIndex) {
                decodedChangeResults[logIndex] = changeResult[logIndex];
                if (typeof changeResult[logIndex] === 'object') {
                  decodedChangeResults[logIndex].data = self.options.decoder(decodedChangeResults[logIndex].data);
                }
              });
            } catch (decodingErrorMesage) {
              decodingError = new Error('[ethjs-filter] while decoding filter change event data from RPC \'' + JSON.stringify(decodedChangeResults) + '\': ' + decodingErrorMesage);
            }
          }

          Object.keys(self.watchers).forEach(function (id) {
            var watcher = self.watchers[id];
            if (watcher.stop === true) {
              delete self.watchers[id];
              return;
            }

            if (decodingError) {
              watcher.callback(decodingError, null);
            } else {
              if (changeError) {
                watcher.callback(changeError, null);
              } else if (Array.isArray(decodedChangeResults) && changeResult.length > 0) {
                watcher.callback(changeError, decodedChangeResults);
              }
            }
          });
        });
      }
    }, self.options.delay);
  }

  Filter.prototype.at = function atFilter(filterId) {
    var self = this;
    self.filterId = filterId;
  };

  Filter.prototype.watch = function watchFilter(watchCallbackInput) {
    var callback = watchCallbackInput || function () {}; // eslint-disable-line
    var self = this;
    var id = Math.random().toString(36).substring(7);
    self.watchers[id] = { callback: callback, stop: false, stopWatching: function stopWatching() {
        self.watchers[id].stop = true;
      } };

    return self.watchers[id];
  };

  Filter.prototype.uninstall = function uninstallFilter(cb) {
    var self = this;
    var callback = cb || null;
    self.watchers = Object.assign({});
    clearInterval(self.interval);

    var prom = new Promise(function (resolve, reject) {
      query.uninstallFilter(self.filterId, function (uninstallError, uninstallResilt) {
        if (uninstallError) {
          reject(uninstallError);
        } else {
          resolve(uninstallResilt);
        }
      });
    });

    if (callback) {
      prom.then(function (res) {
        return callback(null, res);
      })['catch'](function (err) {
        return callback(err, null);
      });
    }

    return callback ? null : prom;
  };

  Filter.prototype['new'] = function newFilter() {
    var callback = null; // eslint-disable-line
    var self = this;
    var filterInputs = [];
    var args = [].slice.call(arguments); // eslint-disable-line
    // pop callback if provided
    if (typeof args[args.length - 1] === 'function') {
      callback = args.pop();
    }

    // if a param object was presented, push that into the inputs
    if (filterName === 'Filter') {
      filterInputs.push(Object.assign(self.options.defaultFilterObject, args[args.length - 1] || {}));
    }

    var prom = new Promise(function (resolve, reject) {
      // add complex callback
      filterInputs.push(function (setupError, filterId) {
        if (!setupError) {
          self.filterId = filterId;
          resolve(filterId);
        } else {
          reject(setupError);
        }
      });

      // apply filter, call new.. filter method
      query['new' + filterName].apply(query, filterInputs);
    });

    if (callback) {
      prom.then(function (res) {
        return callback(null, res);
      })['catch'](function (err) {
        return callback(err, null);
      });
    }

    return callback ? null : prom;
  };

  return Filter;
}

/**
 * EthFilter constructor, intakes a query, helps manage filter event polling
 *
 * @method EthFilter
 * @param {Object} query the `ethjs-query` or `eth-query` object
 * @returns {Object} output an EthFilter instance
 * @throws error if new is not used
 */

function EthFilter(query) {
  var self = this;
  if (!(self instanceof EthFilter)) {
    throw new Error('the EthFilter object must be instantiated with `new` flag.. (e.g. `const filters = new EthFilter(query);`)');
  }
  if (typeof query !== 'object') {
    throw new Error('the EthFilter object must be instantiated with an EthQuery instance (e.g. `const filters = new EthFilter(new EthQuery(provider));`). See github.com/ethjs/ethjs-query for more details..');
  }

  self.Filter = constructFilter('Filter', query);
  self.BlockFilter = constructFilter('BlockFilter', query);
  self.PendingTransactionFilter = constructFilter('PendingTransactionFilter', query);
}

// export EthFilter
module.exports = EthFilter;

/***/ }
/******/ ])
});
;
//# sourceMappingURL=ethjs-filter.js.map