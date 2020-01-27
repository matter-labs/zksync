"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
var BN = require("bn.js");
var ethers_1 = require("ethers");
var utils_1 = require("ethers/utils");
exports.IERC20_INTERFACE = new ethers_1.utils.Interface(require("../abi/IERC20.json").interface);
exports.SYNC_MAIN_CONTRACT_INTERFACE = new ethers_1.utils.Interface(require("../abi/SyncMain.json").interface);
exports.SYNC_PRIOR_QUEUE_INTERFACE = new ethers_1.utils.Interface(require("../abi/SyncPriorityQueue.json").interface);
exports.SYNC_GOV_CONTRACT_INTERFACE = new ethers_1.utils.Interface(require("../abi/SyncGov.json").interface);
var AMOUNT_EXPONENT_BIT_WIDTH = 5;
var AMOUNT_MANTISSA_BIT_WIDTH = 35;
var FEE_EXPONENT_BIT_WIDTH = 5;
var FEE_MANTISSA_BIT_WIDTH = 11;
function floatToInteger(floatBytes, expBits, mantissaBits, expBaseNumber) {
    if (floatBytes.length * 8 != mantissaBits + expBits) {
        throw new Error("Float unpacking, incorrect input length");
    }
    var floatHolder = new BN(floatBytes, 16, "be"); // keep bit order
    var expBase = new BN(expBaseNumber);
    var exponent = new BN(0);
    var expPow2 = new BN(1);
    var two = new BN(2);
    for (var i = 0; i < expBits; i++) {
        if (floatHolder.testn(i)) {
            exponent = exponent.add(expPow2);
        }
        expPow2 = expPow2.mul(two);
    }
    exponent = expBase.pow(exponent);
    var mantissa = new BN(0);
    var mantissaPow2 = new BN(1);
    for (var i = expBits; i < expBits + mantissaBits; i++) {
        if (floatHolder.testn(i)) {
            mantissa = mantissa.add(mantissaPow2);
        }
        mantissaPow2 = mantissaPow2.mul(two);
    }
    return exponent.mul(mantissa);
}
exports.floatToInteger = floatToInteger;
function bitsIntoBytesInBEOrder(bits) {
    if (bits.length % 8 != 0) {
        throw new Error("wrong number of bits to pack");
    }
    var nBytes = bits.length / 8;
    var resultBytes = Buffer.alloc(nBytes, 0);
    for (var byte = 0; byte < nBytes; ++byte) {
        var value = 0;
        if (bits[byte * 8]) {
            value |= 0x80;
        }
        if (bits[byte * 8 + 1]) {
            value |= 0x40;
        }
        if (bits[byte * 8 + 2]) {
            value |= 0x20;
        }
        if (bits[byte * 8 + 3]) {
            value |= 0x10;
        }
        if (bits[byte * 8 + 4]) {
            value |= 0x08;
        }
        if (bits[byte * 8 + 5]) {
            value |= 0x04;
        }
        if (bits[byte * 8 + 6]) {
            value |= 0x02;
        }
        if (bits[byte * 8 + 7]) {
            value |= 0x01;
        }
        resultBytes[byte] = value;
    }
    return resultBytes;
}
exports.bitsIntoBytesInBEOrder = bitsIntoBytesInBEOrder;
function integerToFloat(integer, exp_bits, mantissa_bits, exp_base) {
    var max_exponent = new BN(10).pow(new BN((1 << exp_bits) - 1));
    var max_mantissa = new BN(2).pow(new BN(mantissa_bits)).subn(1);
    if (integer.gt(max_mantissa.mul(max_exponent))) {
        throw new Error("Integer is too big");
    }
    var exponent = 0;
    var mantissa = integer;
    while (mantissa.gt(max_mantissa)) {
        mantissa = mantissa.divn(exp_base);
        exponent += 1;
    }
    // encode into bits. First bits of mantissa in LE order
    var encoding = [];
    for (var i = 0; i < exp_bits; ++i) {
        if ((exponent & (1 << i)) == 0) {
            encoding.push(false);
        }
        else {
            encoding.push(true);
        }
    }
    for (var i = 0; i < mantissa_bits; ++i) {
        if (mantissa.testn(i)) {
            encoding.push(true);
        }
        else {
            encoding.push(false);
        }
    }
    return Buffer.from(bitsIntoBytesInBEOrder(encoding.reverse()).reverse());
}
exports.integerToFloat = integerToFloat;
function reverseBits(buffer) {
    var reversed = Buffer.from(buffer.reverse());
    reversed.map(function (b) {
        // reverse bits in byte
        b = ((b & 0xf0) >> 4) | ((b & 0x0f) << 4);
        b = ((b & 0xcc) >> 2) | ((b & 0x33) << 2);
        b = ((b & 0xaa) >> 1) | ((b & 0x55) << 1);
        return b;
    });
    return reversed;
}
exports.reverseBits = reverseBits;
function packAmount(amount) {
    return reverseBits(integerToFloat(amount, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH, 10));
}
function packFee(amount) {
    return reverseBits(integerToFloat(amount, FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, 10));
}
function packAmountChecked(amount) {
    if (closestPackableTransactionAmount(amount.toString()).toString() !==
        amount.toString()) {
        throw new Error("Transaction Amount is not packable");
    }
    return packAmount(amount);
}
exports.packAmountChecked = packAmountChecked;
function packFeeChecked(amount) {
    if (closestPackableTransactionFee(amount.toString()).toString() !==
        amount.toString()) {
        throw new Error("Fee Amount is not packable");
    }
    return packFee(amount);
}
exports.packFeeChecked = packFeeChecked;
/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param amount
 */
function closestPackableTransactionAmount(amount) {
    var amountBN = new BN(ethers_1.utils.bigNumberify(amount).toString());
    var packedAmount = packAmount(amountBN);
    return utils_1.bigNumberify(floatToInteger(packedAmount, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH, 10).toString());
}
exports.closestPackableTransactionAmount = closestPackableTransactionAmount;
/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param fee
 */
function closestPackableTransactionFee(fee) {
    var feeBN = new BN(ethers_1.utils.bigNumberify(fee).toString());
    var packedFee = packFee(feeBN);
    return utils_1.bigNumberify(floatToInteger(packedFee, FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, 10).toString());
}
exports.closestPackableTransactionFee = closestPackableTransactionFee;
function buffer2bitsLE(buff) {
    var res = new Array(buff.length * 8);
    for (var i = 0; i < buff.length; i++) {
        var b = buff[i];
        res[i * 8] = (b & 0x01) != 0;
        res[i * 8 + 1] = (b & 0x02) != 0;
        res[i * 8 + 2] = (b & 0x04) != 0;
        res[i * 8 + 3] = (b & 0x08) != 0;
        res[i * 8 + 4] = (b & 0x10) != 0;
        res[i * 8 + 5] = (b & 0x20) != 0;
        res[i * 8 + 6] = (b & 0x40) != 0;
        res[i * 8 + 7] = (b & 0x80) != 0;
    }
    return res;
}
exports.buffer2bitsLE = buffer2bitsLE;
function buffer2bitsBE(buff) {
    var res = new Array(buff.length * 8);
    for (var i = 0; i < buff.length; i++) {
        var b = buff[i];
        res[i * 8] = (b & 0x80) != 0;
        res[i * 8 + 1] = (b & 0x40) != 0;
        res[i * 8 + 2] = (b & 0x20) != 0;
        res[i * 8 + 3] = (b & 0x10) != 0;
        res[i * 8 + 4] = (b & 0x08) != 0;
        res[i * 8 + 5] = (b & 0x04) != 0;
        res[i * 8 + 6] = (b & 0x02) != 0;
        res[i * 8 + 7] = (b & 0x01) != 0;
    }
    return res;
}
exports.buffer2bitsBE = buffer2bitsBE;
function sleep(ms) {
    return new Promise(function (resolve) { return setTimeout(resolve, ms); });
}
exports.sleep = sleep;
