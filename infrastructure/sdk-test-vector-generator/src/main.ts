// Generator for test vectors to be used by various SDK
import * as fs from "fs";

import { generateCryptoTestVectors } from "./crypto-vector";
import { generateTxEncodingVectors } from "./tx-vector";

export async function generateSDKTestVectors(outputFile: string = "test_vectors.json") {
    const cryptoVectors = await generateCryptoTestVectors();
    const txVectors = await generateTxEncodingVectors();

    const resultTestVector = {
        cryptoPrimitivesTest: cryptoVectors,
        txTest: txVectors,
    };

    const testVectorJSON = JSON.stringify(resultTestVector, null, 2);

    fs.writeFileSync(outputFile, testVectorJSON);
}

(async () => {
    await generateSDKTestVectors();
})();
