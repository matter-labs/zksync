/// <reference types="node" />
import BN = require("bn.js");
import { utils } from "ethers";
export declare const IERC20_INTERFACE: utils.Interface;
export declare const SYNC_MAIN_CONTRACT_INTERFACE: utils.Interface;
export declare const SYNC_PRIOR_QUEUE_INTERFACE: utils.Interface;
export declare const SYNC_GOV_CONTRACT_INTERFACE: utils.Interface;
export declare function floatToInteger(floatBytes: Buffer, expBits: number, mantissaBits: number, expBaseNumber: number): BN;
export declare function bitsIntoBytesInBEOrder(bits: boolean[]): Buffer;
export declare function integerToFloat(integer: BN, exp_bits: number, mantissa_bits: number, exp_base: number): Buffer;
export declare function reverseBits(buffer: Buffer): Buffer;
export declare function packAmountChecked(amount: BN): Buffer;
export declare function packFeeChecked(amount: BN): Buffer;
/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param amount
 */
export declare function closestPackableTransactionAmount(amount: utils.BigNumberish): utils.BigNumber;
/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param fee
 */
export declare function closestPackableTransactionFee(fee: utils.BigNumberish): utils.BigNumber;
export declare function buffer2bitsLE(buff: any): any[];
export declare function buffer2bitsBE(buff: any): any[];
export declare function sleep(ms: any): Promise<unknown>;
