// Generator for test vectors to be used by various SDK
import * as fs from 'fs';

import { generateCryptoTestVectors } from './vectors/crypto-vector';
import { generateTxEncodingVectors } from './vectors/tx-vector';
import { generateUtilsVectors } from './vectors/utils-vector';

export async function generateSDKTestVectors(outputFile: string = 'test-vectors.json') {
    const cryptoVectors = await generateCryptoTestVectors();
    const txVectors = await generateTxEncodingVectors();
    const utilsVectors = generateUtilsVectors();

    const resultTestVector = {
        cryptoPrimitivesTest: cryptoVectors,
        txTest: txVectors,
        utils: utilsVectors
    };

    const testVectorJSON = JSON.stringify(resultTestVector, null, 2);

    fs.writeFileSync(outputFile, testVectorJSON);
}

(async () => {
    await generateSDKTestVectors();
})();
