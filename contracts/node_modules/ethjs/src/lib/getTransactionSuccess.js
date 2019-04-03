module.exports = eth => (txHash, callback) => {
  let count = 0;

  const timeout = eth.options.timeout || 800000;
  const interval = eth.options.interval || 7000;

  const prom = new Promise((resolve, reject) => {
    const txInterval = setInterval(() => {
      eth.getTransactionReceipt(txHash, (err, result) => {
        if (err) {
          clearInterval(txInterval);
          reject(err);
        }

        if (!err && result) {
          clearInterval(txInterval);
          resolve(result);
        }
      });

      if (count >= timeout) {
        clearInterval(txInterval);
        const errMessage = `Receipt timeout waiting for tx hash: ${txHash}`;
        reject(errMessage);
      }

      count += interval;
    }, interval);
  });

  if (callback) {
    prom.then(res => callback(null, res)).catch(err => callback(err, null));
  }

  return callback ? null : prom;
};
