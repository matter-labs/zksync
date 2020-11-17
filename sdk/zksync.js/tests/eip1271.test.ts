import { expect } from 'chai';
import * as ethers from 'ethers';
import { loadTestConfig } from './test-utils';
import * as zkUtils from '../src/utils';

const testConfig = loadTestConfig();
const web3Url = testConfig.eth.web3_url;
const provider = new ethers.providers.JsonRpcProvider(web3Url);
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
