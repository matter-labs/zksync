import * as zksync from 'zksync';
import { TestVector, TestVectorEntry } from '../types';
import { utils } from 'ethers';

/**
 * Utilities test vector consist of several independent test vectors.
 */
export interface UtilsVectors {
    amountPacking: TestVector<PackingTestEntry>;
    feePacking: TestVector<PackingTestEntry>;
    tokenFormatting: TestVector<TokenFormattingEntry>;
}

/**
 * Test vector for packability checks.
 */
export interface PackingTestEntry extends TestVectorEntry {
    inputs: {
        // Value that should be checked to be packable.
        value: string;
    };
    outputs: {
        // Whether provided amount is packable or not.
        packable: boolean;
        // Closest packable value. May be the same as `inputs.value` if value is packable.
        closestPackable: string;
        // Closest packable value packed into an byte array. Represented as a hexadecimal string.
        packedValue: string;
    };
}

/**
 * Test vector for token formatting.
 * Token formatting must be the same, since this algorithm is used in the Ethereum signature messages.
 */
export interface TokenFormattingEntry extends TestVectorEntry {
    inputs: {
        // Token symbol, e.g. "ETH"
        token: string;
        // Amount of token decimals, e.g. 6 or 18
        decimals: number;
        // Amount of token as a string.
        amount: string;
    };
    outputs: {
        // Formatted string, e.g. `0.001 ETH`.
        formatted: string;
    };
}

export function generateUtilsVectors(): UtilsVectors {
    const amountPacking = generateAmountPackingVector();
    const feePacking = generateFeePackingVector();
    const tokenFormatting = generateTokenFormattingVector();

    return {
        amountPacking,
        feePacking,
        tokenFormatting
    };
}

function generateFeePackingVector(): TestVector<PackingTestEntry> {
    const test_vector = ['0', '1000', '1111', '474732833474', '474732833400', '10000000000000'];
    const items = [];
    for (let value of test_vector) {
        const packable = zksync.utils.isTransactionFeePackable(value);
        const closestPackable = zksync.utils.closestPackableTransactionFee(value);
        const packed = zksync.utils.packFeeChecked(closestPackable);
        items.push({
            inputs: {
                value
            },
            outputs: {
                packable,
                closestPackable: closestPackable.toString(),
                packedValue: utils.hexlify(packed)
            }
        });
    }

    const vector = {
        description: 'Checks for fee packing',
        items
    };

    return vector;
}

function generateAmountPackingVector(): TestVector<PackingTestEntry> {
    const testVector = ['0', '1000', '1111', '474732833474', '474732833400', '10000000000000'];
    const items = [];
    for (const value of testVector) {
        const packable = zksync.utils.isTransactionAmountPackable(value);
        const closestPackable = zksync.utils.closestPackableTransactionAmount(value);
        const packed = zksync.utils.packAmountChecked(closestPackable);
        items.push({
            inputs: {
                value
            },
            outputs: {
                packable,
                closestPackable: closestPackable.toString(),
                packedValue: utils.hexlify(packed)
            }
        });
    }

    const vector = {
        description: 'Checks for amount packing',
        items
    };

    return vector;
}

function generateTokenFormattingVector(): TestVector<TokenFormattingEntry> {
    const testVector = [
        { token: 'NNM', decimals: 0, amount: '1000000000000000100000', formatted: '1000000000000000100000.0 NNM' },
        { token: 'DAI', decimals: 6, amount: '1000000', formatted: '1.0 DAI' },
        { token: 'ZRO', decimals: 11, amount: '0', formatted: '0.0 ZRO' },
        { token: 'ETH', decimals: 18, amount: '1000000000000000100000', formatted: '1000.0000000000001 ETH' }
    ];

    const items = [];
    for (const value of testVector) {
        items.push({
            inputs: {
                token: value.token,
                decimals: value.decimals,
                amount: value.amount
            },
            outputs: {
                formatted: value.formatted
            }
        });
    }

    return {
        description: 'Checks for token amount formatting',
        items
    };
}
