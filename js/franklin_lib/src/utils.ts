import BN = require('bn.js');

export function floatToInteger(floatBytes: Buffer, exp_bits: number, mantissa_bits: number, exp_base: number): BN {
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

export function bitsIntoBytesInOrder(bits: Array<boolean>) : Buffer {
    if (bits.length % 8 != 0) {
        throw "wrong number of bits to pack";
    }
    let nBytes = bits.length / 8;
    let resultBytes = Buffer.alloc(nBytes, 0);

    for (let byte = 0; byte < nBytes; ++byte) {
        let value = 0;
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

export function integerToFloat(integer: BN, exp_bits: number, mantissa_bits: number, exp_base: number): Buffer {

    let max_exponent = (new BN(10)).pow(new BN((1 << exp_bits) - 1));
    let max_mantissa = (new BN(2)).pow(new BN(mantissa_bits)).subn(1);

    if (integer.gt(max_mantissa.mul(max_exponent))) {
        throw "Integer is too big";
    }

    let exponent = 0;
    let mantissa = integer;

    if (integer.gt(max_mantissa)) {
        // always try best precision
        let exponent_guess = integer.div(max_mantissa);
        let exponent_temp = exponent_guess;

        while(true) {
            if (exponent_temp.ltn(exp_base)) {
                break;
            }
            exponent_temp = exponent_temp.divn(exp_base);
            exponent += 1;
        }

        exponent_temp = new BN(1);
        for (let i = 0; i < exponent; ++i) {
            exponent_temp = exponent_temp.muln(exp_base);
        }

        if (exponent_temp.mul(max_mantissa) < integer) {
            exponent += 1;
            exponent_temp = exponent_temp.muln(exp_base);
        }

        mantissa = integer.div(exponent_temp);
    }

    // encode into bits. First bits of mantissa in LE order

    let encoding = [];

    for (let i = 0; i < exp_bits; ++i) {
        if ((exponent & (1 << i)) == 0) {
            encoding.push(false);
        } else {
            encoding.push(true);
        }
    }

    for (let i =0; i < mantissa_bits; ++i) {
        if (mantissa.and(new BN(1 << i)).eqn(0)) {
            encoding.push(false);
        } else {
            encoding.push(true);
        }
    }

    return bitsIntoBytesInOrder(encoding.reverse()).reverse();
}

export function reverseBits(buffer: Buffer): Buffer {
    let reversed = buffer.reverse();
    reversed.map( (b, i, a) => {
        // reverse bits in byte
        b = (b & 0xF0) >> 4 | (b & 0x0F) << 4;
        b = (b & 0xCC) >> 2 | (b & 0x33) << 2;
        b = (b & 0xAA) >> 1 | (b & 0x55) << 1;
        return b
    });
    return reversed;
}

export function packAmount(amount: BN): Buffer {
    return reverseBits(integerToFloat(amount, 5, 19, 10));
}

export function packFee(amount: BN): Buffer {
    return reverseBits(integerToFloat(amount, 6, 10, 10));
}
