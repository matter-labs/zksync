import BN = require("bn.js");
import { ethers } from "ethers";

export function floatToInteger(
  floatBytes: Buffer,
  exp_bits: number,
  mantissa_bits: number,
  exp_base: number
): BN {
  const floatHolder = new BN(floatBytes, 16, "be"); // keep bit order
  const totalBits = floatBytes.length * 8 - 1; // starts from zero
  const expBase = new BN(exp_base);
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

export function bitsIntoBytesInBEOrder(bits: boolean[]): Buffer {
  if (bits.length % 8 != 0) {
    throw new Error("wrong number of bits to pack");
  }
  const nBytes = bits.length / 8;
  const resultBytes = Buffer.alloc(nBytes, 0);

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

export function integerToFloat(
  integer: BN,
  exp_bits: number,
  mantissa_bits: number,
  exp_base: number
): Buffer {
  const max_exponent = new BN(10).pow(new BN((1 << exp_bits) - 1));
  const max_mantissa = new BN(2).pow(new BN(mantissa_bits)).subn(1);

  if (integer.gt(max_mantissa.mul(max_exponent))) {
    throw new Error("Integer is too big");
  }

  let exponent = 0;
  let mantissa = integer;
  while (mantissa.gt(max_mantissa)) {
    mantissa = mantissa.divn(exp_base);
    exponent += 1;
  }

  // encode into bits. First bits of mantissa in LE order
  const encoding = [];

  for (let i = 0; i < exp_bits; ++i) {
    if ((exponent & (1 << i)) == 0) {
      encoding.push(false);
    } else {
      encoding.push(true);
    }
  }

  for (let i = 0; i < mantissa_bits; ++i) {
    if (mantissa.and(new BN(1 << i)).eqn(0)) {
      encoding.push(false);
    } else {
      encoding.push(true);
    }
  }

  return Buffer.from(bitsIntoBytesInBEOrder(encoding.reverse()).reverse());
}

export function reverseBits(buffer: Buffer): Buffer {
  const reversed = Buffer.from(buffer.reverse());
  reversed.map(b => {
    // reverse bits in byte
    b = ((b & 0xf0) >> 4) | ((b & 0x0f) << 4);
    b = ((b & 0xcc) >> 2) | ((b & 0x33) << 2);
    b = ((b & 0xaa) >> 1) | ((b & 0x55) << 1);
    return b;
  });
  return reversed;
}

function packAmount(amount: BN): Buffer {
  return reverseBits(integerToFloat(amount, 5, 19, 10));
}

function packFee(amount: BN): Buffer {
  return reverseBits(integerToFloat(amount, 6, 10, 10));
}


export function packAmountChecked(amount: BN): Buffer {
  // TODO: check is amount is packable;
  return packAmount(amount);
}

export function packFeeChecked(amount: BN): Buffer {
  // TODO: check is amount is packable;
  return packFee(amount);
}

/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param amount
 * @param AMOUNT_EXPONENT_BIT_WIDTH
 * @param AMOUNT_MANTISSA_BIT_WIDTH
 */
function packedHelper(
  amount: ethers.utils.BigNumberish,
  AMOUNT_EXPONENT_BIT_WIDTH: number,
  AMOUNT_MANTISSA_BIT_WIDTH: number
) {
  const amountStr10 = ethers.utils.bigNumberify(amount).toString();
  const bn = new BN(amountStr10, 10);

  const packed = integerToFloat(
    bn,
    AMOUNT_EXPONENT_BIT_WIDTH,
    AMOUNT_MANTISSA_BIT_WIDTH,
    10
  );
  const unpacked = floatToInteger(
    packed,
    AMOUNT_EXPONENT_BIT_WIDTH,
    AMOUNT_MANTISSA_BIT_WIDTH,
    10
  );
  return unpacked.toString(10);
}

/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param amount
 */
export function packedAmount(amount: ethers.utils.BigNumberish) {
  const AMOUNT_EXPONENT_BIT_WIDTH = 5;
  const AMOUNT_MANTISSA_BIT_WIDTH = 19;
  return packedHelper(
    amount,
    AMOUNT_EXPONENT_BIT_WIDTH,
    AMOUNT_MANTISSA_BIT_WIDTH
  );
}

/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param fee
 */
export function packedFee(fee: ethers.utils.BigNumberish) {
  const FEE_EXPONENT_BIT_WIDTH = 4;
  const FEE_MANTISSA_BIT_WIDTH = 4;
  return packedHelper(fee, FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH);
}
