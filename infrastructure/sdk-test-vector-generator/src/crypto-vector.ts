import { TestVector, CryptoPrimitivesTestEntry } from "./types";
import * as zksync from "zksync";
import { utils } from "ethers";
import { generateArray } from "./utils";

/**
 * Returns the test vector to generate cryptographic primitives.
 * All the data fields are represented in a hexadecimal form.
 */
export async function generateCryptoTestVectors(): Promise<TestVector<CryptoPrimitivesTestEntry>> {
    const seed = generateArray(32);
    const bytesToSign = generateArray(64);

    const privateKey = await zksync.crypto.privateKeyFromSeed(seed);
    const { pubKey, signature } = await zksync.crypto.signTransactionBytes(privateKey, bytesToSign);

    const item = {
        inputs: {
            seed: utils.hexlify(seed),
            message: utils.hexlify(bytesToSign),
        },
        outputs: {
            privateKey: utils.hexlify(privateKey),
            pubKeyHash: pubKey,
            signature: signature,
        },
    };

    return {
        description: "Contains the seed for private key and the message for signing",
        items: [item],
    };
}
