'use strict';

/* eslint-disable */

var utils = require('./utils/index.js');
var uint256Coder = utils.uint256Coder;
var coderBoolean = utils.coderBoolean;
var coderFixedBytes = utils.coderFixedBytes;
var coderAddress = utils.coderAddress;
var coderDynamicBytes = utils.coderDynamicBytes;
var coderString = utils.coderString;
var coderArray = utils.coderArray;
var paramTypePart = utils.paramTypePart;
var getParamCoder = utils.getParamCoder;

function Result() {}

function encodeParams(types, values) {
  if (types.length !== values.length) {
    throw new Error('[ethjs-abi] while encoding params, types/values mismatch, Your contract requires ' + types.length + ' types (arguments), and you passed in ' + values.length);
  }

  var parts = [];

  types.forEach(function (type, index) {
    var coder = getParamCoder(type);
    parts.push({ dynamic: coder.dynamic, value: coder.encode(values[index]) });
  });

  function alignSize(size) {
    return parseInt(32 * Math.ceil(size / 32));
  }

  var staticSize = 0,
      dynamicSize = 0;
  parts.forEach(function (part) {
    if (part.dynamic) {
      staticSize += 32;
      dynamicSize += alignSize(part.value.length);
    } else {
      staticSize += alignSize(part.value.length);
    }
  });

  var offset = 0,
      dynamicOffset = staticSize;
  var data = new Buffer(staticSize + dynamicSize);

  parts.forEach(function (part, index) {
    if (part.dynamic) {
      uint256Coder.encode(dynamicOffset).copy(data, offset);
      offset += 32;

      part.value.copy(data, dynamicOffset);
      dynamicOffset += alignSize(part.value.length);
    } else {
      part.value.copy(data, offset);
      offset += alignSize(part.value.length);
    }
  });

  return '0x' + data.toString('hex');
}

// decode bytecode data from output names and types
function decodeParams(names, types, data) {
  var useNumberedParams = arguments.length > 3 && arguments[3] !== undefined ? arguments[3] : true;

  // Names is optional, so shift over all the parameters if not provided
  if (arguments.length < 3) {
    data = types;
    types = names;
    names = [];
  }

  data = utils.hexOrBuffer(data);
  var values = new Result();

  var offset = 0;
  types.forEach(function (type, index) {
    var coder = getParamCoder(type);

    if (coder.dynamic) {
      var dynamicOffset = uint256Coder.decode(data, offset);
      var result = coder.decode(data, dynamicOffset.value.toNumber());
      offset += dynamicOffset.consumed;
    } else {
      var result = coder.decode(data, offset);
      offset += result.consumed;
    }

    if (useNumberedParams) {
      values[index] = result.value;
    }

    if (names[index]) {
      values[names[index]] = result.value;
    }
  });
  return values;
}

// create an encoded method signature from an ABI object
function encodeSignature(method) {
  var signature = method.name + '(' + utils.getKeys(method.inputs, 'type').join(',') + ')';
  var signatureEncoded = '0x' + new Buffer(utils.keccak256(signature), 'hex').slice(0, 4).toString('hex');

  return signatureEncoded;
}

// encode method ABI object with values in an array, output bytecode
function encodeMethod(method, values) {
  var paramsEncoded = encodeParams(utils.getKeys(method.inputs, 'type'), values).substring(2);

  return '' + encodeSignature(method) + paramsEncoded;
}

// decode method data bytecode, from method ABI object
function decodeMethod(method, data) {
  var outputNames = utils.getKeys(method.outputs, 'name', true);
  var outputTypes = utils.getKeys(method.outputs, 'type');

  return decodeParams(outputNames, outputTypes, utils.hexOrBuffer(data));
}

// decode method data bytecode, from method ABI object
function encodeEvent(eventObject, values) {
  return encodeMethod(eventObject, values);
}

function eventSignature(eventObject) {
  var signature = eventObject.name + '(' + utils.getKeys(eventObject.inputs, 'type').join(',') + ')';

  return '0x' + utils.keccak256(signature);
}

// decode method data bytecode, from method ABI object
function decodeEvent(eventObject, data, topics) {
  var useNumberedParams = arguments.length > 3 && arguments[3] !== undefined ? arguments[3] : true;

  var nonIndexed = eventObject.inputs.filter(function (input) {
    return !input.indexed;
  });
  var nonIndexedNames = utils.getKeys(nonIndexed, 'name', true);
  var nonIndexedTypes = utils.getKeys(nonIndexed, 'type');
  var event = decodeParams(nonIndexedNames, nonIndexedTypes, utils.hexOrBuffer(data), useNumberedParams);
  var topicOffset = eventObject.anonymous ? 0 : 1;

  eventObject.inputs.filter(function (input) {
    return input.indexed;
  }).map(function (input, i) {
    var topic = new Buffer(topics[i + topicOffset].slice(2), 'hex');
    var coder = getParamCoder(input.type);
    event[input.name] = coder.decode(topic, 0).value;
  });

  event._eventName = eventObject.name;

  return event;
}

// Decode a specific log item with a specific event abi
function decodeLogItem(eventObject, log) {
  var useNumberedParams = arguments.length > 2 && arguments[2] !== undefined ? arguments[2] : true;

  if (eventObject && log.topics[0] === eventSignature(eventObject)) {
    return decodeEvent(eventObject, log.data, log.topics, useNumberedParams);
  }
}

// Create a decoder for all events defined in an abi. It returns a function which is called
// on an array of log entries such as received from getLogs or getTransactionReceipt and parses
// any matching log entries
function logDecoder(abi) {
  var useNumberedParams = arguments.length > 1 && arguments[1] !== undefined ? arguments[1] : true;

  var eventMap = {};
  abi.filter(function (item) {
    return item.type === 'event';
  }).map(function (item) {
    eventMap[eventSignature(item)] = item;
  });
  return function (logItems) {
    return logItems.map(function (log) {
      return decodeLogItem(eventMap[log.topics[0]], log, useNumberedParams);
    }).filter(function (i) {
      return i;
    });
  };
}

module.exports = {
  encodeParams: encodeParams,
  decodeParams: decodeParams,
  encodeMethod: encodeMethod,
  decodeMethod: decodeMethod,
  encodeEvent: encodeEvent,
  decodeEvent: decodeEvent,
  decodeLogItem: decodeLogItem,
  logDecoder: logDecoder,
  eventSignature: eventSignature,
  encodeSignature: encodeSignature
};