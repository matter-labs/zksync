import assert from 'assert';
import BN = require('bn.js');

export function floatToInteger(floatBytes: Buffer, exp_bits: number, mantissa_bits: number, exp_base: number): BN {
    assert(floatBytes.length * 8 == exp_bits + mantissa_bits);
    const floatHolder = new BN(floatBytes, 16, 'be'); // keep bit order
    const totalBits = floatBytes.length * 8 - 1; // starts from zero
    let expBase = new BN(exp_base);
    let exponent = new BN(0);
    let exp_power_of_to = new BN(1);
    const two = new BN(2);
    for (let i = 0; i < exp_bits; i++) {
        if (floatHolder.testn(totalBits - i)) {
            exponent = exponent.add(exp_power_of_to);
        }
        exp_power_of_to = exp_power_of_to.mul(two);
    }
    exponent = expBase.pow(exponent);
    let mantissa = new BN(0);
    let mantissa_power_of_to = new BN(1);
    for (let i = 0; i < mantissa_bits; i++) {
        if (floatHolder.testn(totalBits - exp_bits - i)) {
            mantissa = mantissa.add(mantissa_power_of_to);
        }
        mantissa_power_of_to = mantissa_power_of_to.mul(two);
    }
    return exponent.mul(mantissa);
}

export function integerToFloat(integer: BN, exp_bits: number, mantissa_bits: number, exp_base: number): Buffer {
    function integerToFloatInner(integer, exp_bits, mantissa_bits, exp_base, second_pass) {
        // change strategy. First try to guess the precision, and then reparse;
        const maxMantissa = new BN(1).ushln(mantissa_bits).subn(1);
        const maxExponent = new BN(exp_base).pow(new BN(1).ushln(exp_bits).subn(1));
        assert(integer.lte(maxMantissa.mul(maxExponent)));
        // try to get the best precision
        const exponentBase = new BN(exp_base);
        let exponent = new BN(0);
        let one = new BN(1);
        if (integer.gt(maxMantissa)) {
            let exponentGuess = integer.div(maxMantissa);
            let exponentTmp = exponentGuess;

            while (exponentTmp.gte(exponentBase)) {
                exponentTmp = exponentTmp.div(exponentBase);
                exponent = exponent.addn(1);
            }
        }

        let exponentTmp = exponentBase.pow(exponent);
        if (maxMantissa.mul(exponentTmp).lt(integer)) {
            exponent = exponent.addn(1);
        }

        let power = exponentBase.pow(exponent);
        let mantissa = integer.div(power);
        if (!second_pass) {
            let down_to_precision = mantissa.mul(power);
            return integerToFloatInner(down_to_precision, exp_bits, mantissa_bits, exp_base, true);
        }
        // pack
        assert((mantissa_bits + exp_bits) % 8 === 0);
        const totalBits = mantissa_bits + exp_bits - 1;
        const encoding = new BN(0);
        for (let i = 0; i < exp_bits; i++) {
            if (exponent.testn(i)) {
                encoding.bincn(totalBits - i);
            }
        }
        for (let i = 0; i < mantissa_bits; i++) {
            if (mantissa.testn(i)) {
                encoding.bincn(totalBits - exp_bits - i);
            }
        }
        return encoding.toArrayLike(Buffer, 'be', (exp_bits + mantissa_bits) / 8);
    }
    return integerToFloatInner(integer, exp_bits, mantissa_bits, exp_base, false);
}

export function packAmount(amount: BN): Buffer {
    return integerToFloat(amount, 5, 19, 10);
}

export function packFee(amount: BN): Buffer {
    return integerToFloat(amount, 4, 4, 10);
}
