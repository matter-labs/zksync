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

    it('Test EIP1271 prefix detection', async () => {
        const initialMessage = 'hello-world';
        const initialMessageBytes = ethers.utils.toUtf8Bytes(initialMessage);
        const message = zkUtils.getSignedBytesFromMessage(initialMessage, false);

        // Sign with prefix.
        const signaturePrefixed = await zkUtils.signMessagePersonalAPI(ethSigner, message);
        const signatureTypePrefixed = await zkUtils.getEthSignatureType(
            provider,
            message,
            signaturePrefixed,
            testConfig.eip1271.contract_address
        );

        expect(signatureTypePrefixed.verificationMethod, 'Invalid signature type').to.eq('ERC-1271');
        expect(signatureTypePrefixed.isSignedMsgPrefixed, 'Invalid prefix type detected').to.be.true;

        // Sign without prefix. Ethers signer doesn't allow us to sign a message without prefix prepended,
        // so we manually create a private key and sign the hash of the message.
        const pkSigner = new ethers.utils.SigningKey(testConfig.eip1271.owner_private_key);
        const messageHash = ethers.utils.arrayify(ethers.utils.keccak256(message));
        const signatureNotPrefixed = ethers.utils.joinSignature(pkSigner.signDigest(messageHash));
        const signatureTypeNotPrefixed = await zkUtils.getEthSignatureType(
            provider,
            message,
            signatureNotPrefixed,
            testConfig.eip1271.contract_address
        );

        // Sanity check. We've signed the message manually, so it's better to check that it's correct...
        const signatureNotPrefixedValid = await zkUtils.verifyERC1271Signature(
            testConfig.eip1271.contract_address,
            initialMessageBytes,
            signatureNotPrefixed,
            ethSigner,
            false
        );
        expect(signatureNotPrefixedValid, 'Not prefixed EIP1271 signature is invalid').to.be.true;

        expect(signatureTypeNotPrefixed.verificationMethod, 'Invalid signature type').to.eq('ERC-1271');
        expect(signatureTypeNotPrefixed.isSignedMsgPrefixed, 'Invalid prefix type detected').to.be.false;
    });
});
