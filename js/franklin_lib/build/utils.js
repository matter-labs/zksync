"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
var BN = require("bn.js");
var ethers_1 = require("ethers");
function floatToInteger(floatBytes, exp_bits, mantissa_bits, exp_base) {
    var floatHolder = new BN(floatBytes, 16, 'be'); // keep bit order
    var totalBits = floatBytes.length * 8 - 1; // starts from zero
    var expBase = new BN(exp_base);
    var exponent = new BN(0);
    var exp_power_of_to = new BN(1);
    var two = new BN(2);
    for (var i = 0; i < exp_bits; i++) {
        if (floatHolder.testn(totalBits - i)) {
            exponent = exponent.add(exp_power_of_to);
        }
        exp_power_of_to = exp_power_of_to.mul(two);
    }
    exponent = expBase.pow(exponent);
    var mantissa = new BN(0);
    var mantissa_power_of_to = new BN(1);
    for (var i = 0; i < mantissa_bits; i++) {
        if (floatHolder.testn(totalBits - exp_bits - i)) {
            mantissa = mantissa.add(mantissa_power_of_to);
        }
        mantissa_power_of_to = mantissa_power_of_to.mul(two);
    }
    return exponent.mul(mantissa);
}
exports.floatToInteger = floatToInteger;
function bitsIntoBytesInOrder(bits) {
    if (bits.length % 8 != 0) {
        throw "wrong number of bits to pack";
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
exports.bitsIntoBytesInOrder = bitsIntoBytesInOrder;
function integerToFloat(integer, exp_bits, mantissa_bits, exp_base) {
    var max_exponent = (new BN(10)).pow(new BN((1 << exp_bits) - 1));
    var max_mantissa = (new BN(2)).pow(new BN(mantissa_bits)).subn(1);
    if (integer.gt(max_mantissa.mul(max_exponent))) {
        throw "Integer is too big";
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
        if (mantissa.and(new BN(1 << i)).eqn(0)) {
            encoding.push(false);
        }
        else {
            encoding.push(true);
        }
    }
    return bitsIntoBytesInOrder(encoding.reverse()).reverse();
}
exports.integerToFloat = integerToFloat;
function reverseBits(buffer) {
    var reversed = buffer.reverse();
    reversed.map(function (b, i, a) {
        // reverse bits in byte
        b = (b & 0xF0) >> 4 | (b & 0x0F) << 4;
        b = (b & 0xCC) >> 2 | (b & 0x33) << 2;
        b = (b & 0xAA) >> 1 | (b & 0x55) << 1;
        return b;
    });
    return reversed;
}
exports.reverseBits = reverseBits;
function packAmount(amount) {
    return reverseBits(integerToFloat(amount, 5, 19, 10));
}
exports.packAmount = packAmount;
function packFee(amount) {
    return reverseBits(integerToFloat(amount, 6, 10, 10));
}
exports.packFee = packFee;
/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param amount
 * @param AMOUNT_EXPONENT_BIT_WIDTH
 * @param AMOUNT_MANTISSA_BIT_WIDTH
 */
function packedHelper(amount, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH) {
    var amountStr10 = ethers_1.ethers.utils.bigNumberify(amount).toString();
    var bn = new BN(amountStr10, 10);
    var packed = integerToFloat(bn, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH, 10);
    var unpacked = floatToInteger(packed, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH, 10);
    return unpacked.toString(10);
}
/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param amount
 */
function packedAmount(amount) {
    var AMOUNT_EXPONENT_BIT_WIDTH = 5;
    var AMOUNT_MANTISSA_BIT_WIDTH = 19;
    return packedHelper(amount, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH);
}
exports.packedAmount = packedAmount;
/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param fee
 */
function packedFee(fee) {
    var FEE_EXPONENT_BIT_WIDTH = 4;
    var FEE_MANTISSA_BIT_WIDTH = 4;
    return packedHelper(fee, FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH);
}
exports.packedFee = packedFee;
