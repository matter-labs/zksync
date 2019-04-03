function constructFilter(filterName, query) {
  function Filter(options) {
    const self = this;
    self.filterId = null;
    self.options = Object.assign({
      delay: 300,
      decoder: function decodeData(data) { return data; },
      defaultFilterObject: {},
    }, options || {});

    self.watchers = {};
    self.interval = setInterval(() => {
      if (self.filterId !== null && Object.keys(self.watchers).length > 0) {
        query.getFilterChanges(self.filterId, (changeError, changeResult) => {
          const decodedChangeResults = [];
          var decodingError = null; // eslint-disable-line

          if (!changeError) {
            try {
              changeResult.forEach((log, logIndex) => {
                decodedChangeResults[logIndex] = changeResult[logIndex];
                if (typeof changeResult[logIndex] === 'object') {
                  decodedChangeResults[logIndex].data = self.options.decoder(decodedChangeResults[logIndex].data);
                }
              });
            } catch (decodingErrorMesage) {
              decodingError = new Error(`[ethjs-filter] while decoding filter change event data from RPC '${JSON.stringify(decodedChangeResults)}': ${decodingErrorMesage}`);
            }
          }

          Object.keys(self.watchers).forEach((id) => {
            const watcher = self.watchers[id];
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
    const self = this;
    self.filterId = filterId;
  };

  Filter.prototype.watch = function watchFilter(watchCallbackInput) {
    var callback = watchCallbackInput || function() {}; // eslint-disable-line
    const self = this;
    const id = Math.random().toString(36).substring(7);
    self.watchers[id] = { callback, stop: false, stopWatching: (() => {
      self.watchers[id].stop = true;
    }) };

    return self.watchers[id];
  };

  Filter.prototype.uninstall = function uninstallFilter(cb) {
    const self = this;
    const callback = cb || null;
    self.watchers = Object.assign({});
    clearInterval(self.interval);

    const prom = new Promise((resolve, reject) => {
      query.uninstallFilter(self.filterId, (uninstallError, uninstallResilt) => {
        if (uninstallError) {
          reject(uninstallError);
        } else {
          resolve(uninstallResilt);
        }
      });
    });

    if (callback) {
      prom.then(res => callback(null, res)).catch(err => callback(err, null));
    }

    return callback ? null : prom;
  };

  Filter.prototype.new = function newFilter() {
    var callback = null; // eslint-disable-line
    const self = this;
    const filterInputs = [];
    const args = [].slice.call(arguments); // eslint-disable-line
    // pop callback if provided
    if (typeof args[args.length - 1] === 'function') {
      callback = args.pop();
    }

    // if a param object was presented, push that into the inputs
    if (filterName === 'Filter') {
      filterInputs.push(Object.assign(self.options.defaultFilterObject,
        (args[args.length - 1] || {})));
    }

    const prom = new Promise((resolve, reject) => {
      // add complex callback
      filterInputs.push((setupError, filterId) => {
        if (!setupError) {
          self.filterId = filterId;
          resolve(filterId);
        } else {
          reject(setupError);
        }
      });

      // apply filter, call new.. filter method
      query[`new${filterName}`].apply(query, filterInputs);
    });

    if (callback) {
      prom.then(res => callback(null, res)).catch(err => callback(err, null));
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
  const self = this;
  if (!(self instanceof EthFilter)) { throw new Error('the EthFilter object must be instantiated with `new` flag.. (e.g. `const filters = new EthFilter(query);`)'); }
  if (typeof query !== 'object') { throw new Error('the EthFilter object must be instantiated with an EthQuery instance (e.g. `const filters = new EthFilter(new EthQuery(provider));`). See github.com/ethjs/ethjs-query for more details..'); }

  self.Filter = constructFilter('Filter', query);
  self.BlockFilter = constructFilter('BlockFilter', query);
  self.PendingTransactionFilter = constructFilter('PendingTransactionFilter', query);
}

// export EthFilter
module.exports = EthFilter;
