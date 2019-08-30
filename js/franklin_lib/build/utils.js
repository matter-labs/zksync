"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
var BN = require("bn.js");
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
function integerToFloat(integer, exp_bits, mantissa_bits, exp_base) {
    function integerToFloatInner(integer, exp_bits, mantissa_bits, exp_base, second_pass) {
        // change strategy. First try to guess the precision, and then reparse;
        var maxMantissa = new BN(1).ushln(mantissa_bits).subn(1);
        var maxExponent = new BN(exp_base).pow(new BN(1).ushln(exp_bits).subn(1));
        // try to get the best precision
        var exponentBase = new BN(exp_base);
        var exponent = new BN(0);
        var one = new BN(1);
        if (integer.gt(maxMantissa)) {
            var exponentGuess = integer.div(maxMantissa);
            var exponentTmp_1 = exponentGuess;
            while (exponentTmp_1.gte(exponentBase)) {
                exponentTmp_1 = exponentTmp_1.div(exponentBase);
                exponent = exponent.addn(1);
            }
        }
        var exponentTmp = exponentBase.pow(exponent);
        if (maxMantissa.mul(exponentTmp).lt(integer)) {
            exponent = exponent.addn(1);
        }
        var power = exponentBase.pow(exponent);
        var mantissa = integer.div(power);
        if (!second_pass) {
            var down_to_precision = mantissa.mul(power);
            return integerToFloatInner(down_to_precision, exp_bits, mantissa_bits, exp_base, true);
        }
        // pack
        var totalBits = mantissa_bits + exp_bits - 1;
        var encoding = new BN(0);
        //todo: it is probably enough to use 'le' here
        for (var i = mantissa_bits; i > 0; i--) {
            if (mantissa.testn(i)) {
                encoding.bincn(totalBits - exp_bits - i);
            }
        }
        for (var i = exp_bits; i > 0; i--) {
            if (exponent.testn(i)) {
                encoding.bincn(totalBits - i);
            }
        }
        return encoding.toArrayLike(Buffer, 'be', (exp_bits + mantissa_bits) / 8);
    }
    return integerToFloatInner(integer, exp_bits, mantissa_bits, exp_base, false);
}
exports.integerToFloat = integerToFloat;
function packAmount(amount) {
    return integerToFloat(amount, 5, 19, 10);
}
exports.packAmount = packAmount;
function packFee(amount) {
    return integerToFloat(amount, 4, 4, 10);
}
exports.packFee = packFee;
