const crypto = require('crypto');

if (!global.crypto) {
  global.crypto = {};
}

if (!global.crypto.getRandomValues) {
  global.crypto.getRandomValues = function (arr) {
    const buffer = crypto.randomBytes(arr.length);

    return buffer.reduce((arr, value, index) => {
      arr[index] = value;

      return arr;
    }, arr);
  };
}
