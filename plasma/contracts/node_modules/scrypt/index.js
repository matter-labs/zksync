"use strict";

var scryptNative = require("./build/Release/scrypt")
  , Crypto = require("crypto")
  , Os = require("os");

var checkNumberOfArguments = function(args, message, numberOfArguments) {
  if (message === undefined) message = "No arguments present";
  if (numberOfArguments === undefined) numberOfArguments = 1;

  if (args.length < numberOfArguments) {
    var error = new SyntaxError(message);
    throw error;
  }
}

//
// Checks async arguments. Will throw error if callback does not exist and
// promises are not available
//
var checkAsyncArguments = function(args, callback_least_needed_pos, message) {
  checkNumberOfArguments(args);

  var callback_index = (function(){
    for (var i=0; i < args.length; i++) {
      if (typeof args[i] === "function") {
        return i;
      }
    }
  })();

  if (callback_index === undefined) {
    if (typeof Promise !== "undefined")
      return undefined; // if promises are available, don't worry about call backs

    var error = new SyntaxError("No callback function present, and Promises are not available");
    throw error;
  }

  if (callback_index < callback_least_needed_pos) {
    var error = new SyntaxError(message);
    throw error;
  }

  return callback_index;
}

//
// Checks the scrypt parameters object
//
var checkScryptParametersObject = function(params) {
  var error = undefined;

  if (typeof params !== "object") {
    var error = new TypeError("Scrypt parameters type is incorrect: It must be a JSON object");
  }

  if (!error && !params.hasOwnProperty("N")) {
    var error = new TypeError("Scrypt params object does not have 'N' property present");
  }

  if (!error && params.N !== parseInt(params.N)) {
    var error = new TypeError("Scrypt params object 'N' property is not an integer");
  }

  if (!error && !params.hasOwnProperty("r")) {
    var error = new TypeError("Scrypt params object does not have 'r' property present");
  }

  if (!error && params.r !== parseInt(params.r)) {
    var error = new TypeError("Scrypt params object 'r' property is not an integer");
  }

  if (!error && !params.hasOwnProperty("p")) {
    var error = new TypeError("Scrypt params object does not have 'p' property present");
  }

  if (!error && params.p !== parseInt(params.p)) {
    var error = new TypeError("Scrypt params object 'p' property is not an integer");
  }

  if (error) {
    error.propertyName = "Scrypt parameters object";
    error.propertyValue = params;
    throw error;
  }
}

var processParamsArguments = function(args) {
  var error = undefined;

  checkNumberOfArguments(args, "At least one argument is needed - the maxtime", 1);

  // Set defaults (if necessary)
  if (args[1] === undefined) args[1] = 0; //maxmem default to 0
  if (args[2] === undefined) args[2] = 0.5; //max_memfrac default to 0.5

  for(var i=0; i < Math.min(3, args.length); i++) {
    var propertyName = (function() {
      if (i === 0) return "maxtime";
      if (i === 1) return "maxmem";
      if (i === 2) return "max_memfrac";
    })();

    // All args must be of type number
    if (!error && typeof args[i] !== "number") {
      error = new TypeError(propertyName + " must be a number");
    }

    // Specific argument checks
    if (!error) {
      switch (i) {
        case 0: //maxtime
          if (args[0] <= 0) {
            error = new RangeError(propertyName + " must be greater than 0");
          }
          break;

        case 1: //maxmem
          if (args[1] !== parseInt(args[1], 10)) {
            error = new TypeError(propertyName + " must be an integer");
          }

          if (!error && args[1] < 0) {
            error = new RangeError(propertyName + " must be greater than or equal to 0")
          }
          break;

        case 2: //max_memfrac
          if (args[2] < 0.0 || args[2] > 1.0) {
            error = new RangeError(propertyName + " must be between 0.0 and 1.0 inclusive")
          }
          break;
      }
    }

    // Throw error if necessary
    if (error) {
      error.propertyName = propertyName;
      error.propertyValue = args[i];
      throw error;
    }
  }

  return args;
}

var processKDFArguments = function(args) {
  checkNumberOfArguments(args, "At least two arguments are needed - the key and the Scrypt paramaters object", 2)

  //
  // Check key argument
  //
  if (typeof args[0] === "string")
    // Convert string to buffer (if necessary)
    args[0] = new Buffer(args[0]);
  else if (!Buffer.isBuffer(args[0])) {
    var error = new TypeError("Key type is incorrect: It can only be of type string or Buffer");
    error.propertyName = "key";
    error.propertyValue = args[0];
    throw error;
  }

  //
  // Check Scrypt Parameters object
  //
  checkScryptParametersObject(args[1])

  return args;
}

var processVerifyArguments = function(args) {
  checkNumberOfArguments(args, "At least two arguments are needed - the KDF and the key", 2);

  //
  // Check KDF
  //
  if (typeof args[0] === "string")
    // Convert string to buffer (if necessary)
    args[0] = new Buffer(args[0]);
  else if (!Buffer.isBuffer(args[0])) {
    var error = new TypeError("KDF type is incorrect: It can only be of type string or Buffer");
    error.propertyName = "KDF";
    error.propertyValue = args[0];
    throw error;
  }

  //
  // Check Key
  //
  if (typeof args[1] === "string")
    // Convert string to buffer (if necessary)
    args[1] = new Buffer(args[1]);
  else if (!Buffer.isBuffer(args[1])) {
    var error = new TypeError("Key type is incorrect: It can only be of type string or Buffer");
    error.propertyName = "key";
    error.propertyValue = args[1];
    throw error;
  }

  return args;
}

var processHashArguments = function(args) {
  checkNumberOfArguments(args, "At least four arguments are needed - the key to hash, the scrypt params object, the output length of the hash and the salt", 4);

  //
  // Check Key
  //
  if (typeof args[0] === "string")
    // Convert string to buffer (if necessary)
    args[0] = new Buffer(args[0]);
  else if (!Buffer.isBuffer(args[0])) {
    var error = new TypeError("Key type is incorrect: It can only be of type string or Buffer");
    error.propertyName = "KDF";
    error.propertyValue = args[0];
    throw error;
  }

  //
  // Check Scrypt Parameters object
  //
  checkScryptParametersObject(args[1])

  //
  // Check the hash output length
  //
  if (typeof args[2] !== "number" || args[2] !== parseInt(args[2],10)) {
    error = new TypeError("Hash length must be an integer");
    throw error;
  }

  //
  // Check Salt
  //
  if (typeof args[3] === "string")
    // Convert string to buffer (if necessary)
    args[3] = new Buffer(args[3]);
  else if (!Buffer.isBuffer(args[3])) {
    var error = new TypeError("Salt type is incorrect: It can only be of type string or Buffer");
    error.propertyName = "salt";
    error.propertyValue = args[3];
    throw error;
  }

  return args;
}

//
// Scrypt Object
//
var scrypt = {
  paramsSync: function() {
    var args = processParamsArguments(arguments);
    return scryptNative.paramsSync(args[0], args[1], args[2], Os.totalmem());
  },

  params: function() {
    var args = arguments
      , callback_index = checkAsyncArguments(args, 1, "At least one argument is needed before the callback - the maxtime");

    if (callback_index === undefined) {
      // Promise
      return new Promise(function(resolve, reject) {
        args = processParamsArguments(args);
        scryptNative.params(args[0], args[1], args[2],  Os.totalmem(), function(err, params) {
          if (err) {
            reject(err);
          } else {
            resolve(params);
          }
        });
      })
    } else {
      // Normal async with callback

      // If not using promise (so using callback),
      // remove callback function from args and
      // put it in it's own variable. This allows
      // sync check to be used (DRY)
      var callback = args[callback_index];
      delete args[callback_index];
      args = processParamsArguments(args);
      args[3] = callback;
      scryptNative.params(args[0], args[1], args[2],  Os.totalmem(), args[3]);
    }
  },

  kdfSync: function() {
    var args = processKDFArguments(arguments);
    return scryptNative.kdfSync(args[0], args[1], Crypto.randomBytes(256));
  },

  kdf: function() {
    var args = arguments
      , callback_index = checkAsyncArguments(args, 2, "At least two arguments are needed before the call back function - the key and the Scrypt parameters object")
      , that = this;

    args = processKDFArguments(args);

    if (callback_index === undefined) { // promise
      return new Promise(function(resolve, reject) {

        // Get some async salt
        Crypto.randomBytes(256, function(err, salt) {
          if (err) reject(err);
          else {
            scryptNative.kdf(args[0], args[1], salt, function(err, kdfResult) {
              if (err) {
                reject(err);
              } else {
                resolve(kdfResult);
              }
            });
          }
        });
      });
    } else { // Normal async with callback
      Crypto.randomBytes(256, function(err, salt) {
        // Normal async with callback
        if (err) // Crypto.randomBytes err
          args[2](err); // call callback with error
        else
          scryptNative.kdf(args[0], args[1], salt, args[2]);
      });
    }
  },

  verifyKdfSync: function() {
    var args = processVerifyArguments(arguments);
    return scryptNative.verifySync(args[0], args[1]);
  },

  verifyKdf: function() {
    var args = arguments
      , callback_index = checkAsyncArguments(args, 2, "At least two arguments are needed before the callback function - the KDF and the key");

    if (callback_index === undefined) {
      // Promise
      return new Promise(function(resolve, reject) {
        args = processVerifyArguments(args);
        scryptNative.verify(args[0], args[1], function(err, match) {
          if (err) {
            reject(err);
          } else {
            resolve(match);
          }
        });
      })
    } else {
      // Normal async with callback
      args = processVerifyArguments(args);
      scryptNative.verify(args[0], args[1], args[2]);
    }
  },

  hashSync: function() {
    var args = processHashArguments(arguments);
    return scryptNative.hashSync(args[0], args[1], args[2], args[3]);
  },

  hash: function() {
    var args = arguments
      , callback_index = checkAsyncArguments(args, 4, "At least four arguments are needed before the callback - the key to hash, the scrypt params object, the output length of the hash and the salt");

    args = processHashArguments(args);

    if (callback_index === undefined) {
      //Promise
      return new Promise(function(resolve, reject) {
        scryptNative.hash(args[0], args[1], args[2], args[3], function(err, hash) {
          if (err) {
            reject(err);
          } else {
            resolve(hash);
          }
        });
      });
    } else {
      // Normal async with callback
      scryptNative.hash(args[0], args[1], args[2], args[3], args[4]);
    }
  }
};

module.exports = scrypt;
