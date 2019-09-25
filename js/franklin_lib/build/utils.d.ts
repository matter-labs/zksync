/// <reference types="node" />
import BN = require('bn.js');
import { ethers } from 'ethers';
export declare function floatToInteger(floatBytes: Buffer, exp_bits: number, mantissa_bits: number, exp_base: number): BN;
export declare function bitsIntoBytesInOrder(bits: Array<boolean>): Buffer;
export declare function integerToFloat(integer: BN, exp_bits: number, mantissa_bits: number, exp_base: number): Buffer;
export declare function reverseBits(buffer: Buffer): Buffer;
export declare function packAmount(amount: BN): Buffer;
export declare function packFee(amount: BN): Buffer;
/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param amount
 */
export declare function packedAmount(amount: ethers.utils.BigNumberish): string;
/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param fee
 */
export declare function packedFee(fee: ethers.utils.BigNumberish): string;
