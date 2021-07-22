import { expect } from 'chai';
import * as ethers from 'ethers';
import { loadTestConfig } from 'reading-tool';
import * as zkUtils from '../src/utils';

const testConfig = loadTestConfig(false);
const provider = new ethers.providers.JsonRpcProvider(process.env.ETH_CLIENT_WEB3_URL.split(',')[0]);
const ethSigner = new ethers.Wallet(testConfig.eip1271.owner_private_key).connect(provider);

describe('EIP1271 signature check', function () {
    it('Test EIP1271 signature', async () => {
        const initialMessage = 'hello-world';
        const initialMessageBytes = ethers.utils.toUtf8Bytes(initialMessage);
        const message = zkUtils.getSignedBytesFromMessage(initialMessage, false);

        const signature = await zkUtils.signMessagePersonalAPI(ethSigner, message);
        const signatureValid = await zkUtils.verifyERC1271Signature(
            testConfig.eip1271.contract_address,
            initialMessageBytes,
            signature,
            ethSigner
        );

        expect(signatureValid, 'EIP1271 signature is invalid').to.be.true;
    });
});
