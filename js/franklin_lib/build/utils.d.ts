/// <reference types="node" />
import BN = require('bn.js');
export declare function floatToInteger(floatBytes: Buffer, exp_bits: number, mantissa_bits: number, exp_base: number): BN;
export declare function integerToFloat(integer: BN, exp_bits: number, mantissa_bits: number, exp_base: number): Buffer;
export declare function packAmount(amount: BN): Buffer;
export declare function packFee(amount: BN): Buffer;
