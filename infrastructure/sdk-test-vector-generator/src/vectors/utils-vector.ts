import * as zksync from "zksync";
import { TestVector, TestVectorEntry } from "../types";

/**
 * Utilities test vector consist of several independent test vectors.
 */
export interface UtilsVectors {
    amountPacking: TestVector<PackingTestEntry>;
    feePacking: TestVector<PackingTestEntry>;
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
    };
}

export function generateUtilsVectors(): UtilsVectors {
    const amountPacking = generateAmountPackingVector();
    const feePacking = generateFeePackingVector();

    return {
        amountPacking,
        feePacking,
    };
}

function generateFeePackingVector(): TestVector<PackingTestEntry> {
    const test_vector = ["0", "1000", "1111", "474732833474", "474732833400", "10000000000000"];
    const items = [];
    for (let value of test_vector) {
        const packable = zksync.utils.isTransactionFeePackable(value);
        const closestPackable = zksync.utils.closestPackableTransactionFee(value);
        items.push({
            inputs: {
                value,
            },
            outputs: {
                packable,
                closestPackable: closestPackable.toString(),
            },
        });
    }

    const vector = {
        description: "Checks for fee packing",
        items,
    };

    return vector;
}

function generateAmountPackingVector(): TestVector<PackingTestEntry> {
    const test_vector = ["0", "1000", "1111", "474732833474", "474732833400", "10000000000000"];
    const items = [];
    for (let value of test_vector) {
        const packable = zksync.utils.isTransactionAmountPackable(value);
        const closestPackable = zksync.utils.closestPackableTransactionAmount(value);
        items.push({
            inputs: {
                value,
            },
            outputs: {
                packable,
                closestPackable: closestPackable.toString(),
            },
        });
    }

    const vector = {
        description: "Checks for amount packing",
        items,
    };

    return vector;
}
