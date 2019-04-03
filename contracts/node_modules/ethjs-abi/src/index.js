/* eslint-disable */

const utils = require('./utils/index.js');
const uint256Coder = utils.uint256Coder;
const coderBoolean = utils.coderBoolean;
const coderFixedBytes = utils.coderFixedBytes;
const coderAddress = utils.coderAddress;
const coderDynamicBytes = utils.coderDynamicBytes;
const coderString = utils.coderString;
const coderArray = utils.coderArray;
const paramTypePart = utils.paramTypePart;
const getParamCoder = utils.getParamCoder;

function Result() {}

function encodeParams(types, values) {
  if (types.length !== values.length) {
    throw new Error(`[ethjs-abi] while encoding params, types/values mismatch, Your contract requires ${types.length} types (arguments), and you passed in ${values.length}`);
  }

  var parts = [];

  types.forEach(function(type, index) {
    var coder = getParamCoder(type);
    parts.push({dynamic: coder.dynamic, value: coder.encode(values[index])});
  });

  function alignSize(size) {
    return parseInt(32 * Math.ceil(size / 32));
  }

  var staticSize = 0, dynamicSize = 0;
  parts.forEach(function(part) {
    if (part.dynamic) {
      staticSize += 32;
      dynamicSize += alignSize(part.value.length);
    } else {
      staticSize += alignSize(part.value.length);
    }
  });

  var offset = 0, dynamicOffset = staticSize;
  var data = new Buffer(staticSize + dynamicSize);

  parts.forEach(function(part, index) {
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
function decodeParams(names, types, data, useNumberedParams = true) {
  // Names is optional, so shift over all the parameters if not provided
  if (arguments.length < 3) {
    data = types;
    types = names;
    names = [];
  }

  data = utils.hexOrBuffer(data);
  var values = new Result();

  var offset = 0;
  types.forEach(function(type, index) {
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
  const signature = `${method.name}(${utils.getKeys(method.inputs, 'type').join(',')})`;
  const signatureEncoded = `0x${(new Buffer(utils.keccak256(signature), 'hex')).slice(0, 4).toString('hex')}`;

  return signatureEncoded;
}

// encode method ABI object with values in an array, output bytecode
function encodeMethod(method, values) {
  const paramsEncoded = encodeParams(utils.getKeys(method.inputs, 'type'), values).substring(2);

  return `${encodeSignature(method)}${paramsEncoded}`;
}

// decode method data bytecode, from method ABI object
function decodeMethod(method, data) {
  const outputNames = utils.getKeys(method.outputs, 'name', true);
  const outputTypes = utils.getKeys(method.outputs, 'type');

  return decodeParams(outputNames, outputTypes, utils.hexOrBuffer(data));
}

// decode method data bytecode, from method ABI object
function encodeEvent(eventObject, values) {
  return encodeMethod(eventObject, values);
}

function eventSignature(eventObject) {
  const signature = `${eventObject.name}(${utils.getKeys(eventObject.inputs, 'type').join(',')})`;

  return `0x${utils.keccak256(signature)}`;
}

// decode method data bytecode, from method ABI object
function decodeEvent(eventObject, data, topics, useNumberedParams = true) {
  const nonIndexed = eventObject.inputs.filter((input) => !input.indexed)
  const nonIndexedNames = utils.getKeys(nonIndexed, 'name', true);
  const nonIndexedTypes = utils.getKeys(nonIndexed, 'type');
  const event = decodeParams(nonIndexedNames, nonIndexedTypes, utils.hexOrBuffer(data), useNumberedParams);
  const topicOffset = eventObject.anonymous ? 0 : 1;

  eventObject.inputs.filter((input) => input.indexed).map((input, i) => {
    const topic = new Buffer(topics[i + topicOffset].slice(2), 'hex');
    const coder = getParamCoder(input.type);
    event[input.name] = coder.decode(topic, 0).value;
  });

  event._eventName = eventObject.name;

  return event;
}

// Decode a specific log item with a specific event abi
function decodeLogItem(eventObject, log, useNumberedParams = true) {
  if (eventObject && log.topics[0] === eventSignature(eventObject)) {
    return decodeEvent(eventObject, log.data, log.topics, useNumberedParams)
  }
}

// Create a decoder for all events defined in an abi. It returns a function which is called
// on an array of log entries such as received from getLogs or getTransactionReceipt and parses
// any matching log entries
function logDecoder(abi, useNumberedParams = true) {
  const eventMap = {}
  abi.filter(item => item.type === 'event').map(item => {
    eventMap[eventSignature(item)] = item
  })
  return function(logItems) {
    return logItems.map(log => decodeLogItem(eventMap[log.topics[0]], log, useNumberedParams)).filter(i => i)
  }
}


module.exports = {
  encodeParams,
  decodeParams,
  encodeMethod,
  decodeMethod,
  encodeEvent,
  decodeEvent,
  decodeLogItem,
  logDecoder,
  eventSignature,
  encodeSignature
};
