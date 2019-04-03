// type Var = {
//   name: String
//   type: "uint256" | "bytes32" | ...
// }
//
// type Method = {
//   name: String
//   inputs: [Var]
//   output: [Var]
//   constant: Bool
//   payable: Bool
// }

var Bytes = require("./bytes");
var Nat = require("./nat");
var keccak256s = require("./hash").keccak256s;

// (type : String), JSType(type) -> {data: Bytes, dynamic: Bool}
//   ABI-encodes a single term.
var encode = function encode(type, value) {
  if (type === "bytes") {
    var length = Bytes.length(value);
    var nextMul32 = (((length - 1) / 32 | 0) + 1) * 32;
    var lengthEncoded = encode("uint256", Nat.fromNumber(length)).data;
    var bytesEncoded = Bytes.padRight(nextMul32, value);
    return { data: Bytes.concat(lengthEncoded, bytesEncoded), dynamic: true };
  } else if (type === "uint256" || type === "bytes32" || type === "address") {
    return { data: Bytes.pad(32, value), dynamic: false };
  } else {
    throw "Eth-lib can't encode ABI type " + type + " yet.";
  }
};

// (method : Method), [JSType(method.inputs[i].type)] -> Bytes
//   ABI-encodes the transaction data to call a method.
var methodData = function methodData(method, params) {
  var methodSig = method.name + "(" + method.inputs.map(function (i) {
    return i.type;
  }).join(",") + ")";
  var methodHash = keccak256s(methodSig).slice(0, 10);
  var encodedParams = params.map(function (param, i) {
    return encode(method.inputs[i].type, param);
  });
  var headBlock = "0x";
  var dataBlock = "0x";
  for (var i = 0; i < encodedParams.length; ++i) {
    if (encodedParams[i].dynamic) {
      var dataLoc = encodedParams.length * 32 + Bytes.length(dataBlock);
      headBlock = Bytes.concat(headBlock, Bytes.pad(32, Nat.fromNumber(dataLoc)));
      dataBlock = Bytes.concat(dataBlock, encodedParams[i].data);
    } else {
      headBlock = Bytes.concat(headBlock, encodedParams[i].data);
    }
  }
  return Bytes.flatten([methodHash, headBlock, dataBlock]);
};

module.exports = {
  encode: encode,
  methodData: methodData
};