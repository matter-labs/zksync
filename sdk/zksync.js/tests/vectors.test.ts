import { assert, expect } from 'chai';
import { BigNumber, utils, Wallet } from 'ethers';
import * as zksync from 'zksync';

import {
    closestPackableTransactionAmount,
    closestPackableTransactionFee,
    isTransactionAmountPackable,
    isTransactionFeePackable,
    packAmountChecked,
    packFeeChecked,
    TokenSet,
    parseHexWithPrefix,
    getTxHash,
    serializeTx
} from '../src/utils';
import { privateKeyFromSeed, signTransactionBytes } from '../src/crypto';
import { loadTestVectorsConfig } from 'reading-tool';
import { MintNFT, WithdrawNFT } from '../src/types';

const vectors = loadTestVectorsConfig();
const cryptoVectors = vectors.cryptoPrimitivesTest;
const utilsVectors = vectors.utils;
const txVectors = vectors.txTest;
const txHashVectors = vectors.txHashTest;

describe('Crypto tests', function () {
    it(cryptoVectors.description, async function () {
        for (const item of cryptoVectors.items) {
            const seed = parseHexWithPrefix(item.inputs.seed);
            const privateKey = await privateKeyFromSeed(seed);
            const message = parseHexWithPrefix(item.inputs.message);
            const signature = await signTransactionBytes(privateKey, message);

            const outputs = item.outputs;
            expect(utils.hexlify(privateKey), 'private key does not match').eq(outputs.privateKey);
            expect(signature.pubKey, 'public key does not match').eq(outputs.pubKey);
            expect(signature.signature, 'signature does not match').eq(outputs.signature);
        }
    });
});

const amountPackingVectors = utilsVectors.amountPacking;
const feePackingVectors = utilsVectors.feePacking;
const tokenFormattingVectors = utilsVectors.tokenFormatting;

describe('Utils tests', function () {
    it(amountPackingVectors.description, function () {
        for (const item of amountPackingVectors.items) {
            const tokenAmount = BigNumber.from(item.inputs.value);

            assert.equal(
                isTransactionAmountPackable(tokenAmount),
                item.outputs.packable,
                `Amount '${tokenAmount}' not packable`
            );
            expect(closestPackableTransactionAmount(tokenAmount)).to.eql(
                BigNumber.from(item.outputs.closestPackable),
                `Closest packable for '${tokenAmount}' does not match`
            );
            if (item.outputs.packable) {
                assert.equal(
                    utils.hexlify(packAmountChecked(tokenAmount)),
                    item.outputs.packedValue,
                    `Packed value for '${tokenAmount}' does not match`
                );
            }
        }
    });

    it(feePackingVectors.description, function () {
        for (const item of feePackingVectors.items) {
            const feeAmount = BigNumber.from(item.inputs.value);

            assert.equal(isTransactionFeePackable(feeAmount), item.outputs.packable, `Fee '${feeAmount}' not packable`);
            expect(closestPackableTransactionFee(feeAmount)).to.eql(
                BigNumber.from(item.outputs.closestPackable),
                `Closest packable for '${feeAmount}' does not match`
            );
            if (item.outputs.packable) {
                assert.equal(
                    utils.hexlify(packFeeChecked(feeAmount)),
                    item.outputs.packedValue,
                    `Packed value for '${feeAmount}' does not match`
                );
            }
        }
    });

    const tokens = {};
    let id = 0;
    for (const item of tokenFormattingVectors.items) {
        const token = item.inputs.token;
        tokens[token] = {
            address: '0x0000000000000000000000000000000000000000',
            id: id,
            symbol: token,
            decimals: item.inputs.decimals
        };
        id++;
    }

    it(tokenFormattingVectors.description, function () {
        const tokenCache = new TokenSet(tokens);

        for (const item of tokenFormattingVectors.items) {
            const unitsStr = tokenCache.formatToken(item.inputs.token, item.inputs.amount);
            expect(`${unitsStr} ${item.inputs.token}`).to.eql(item.outputs.formatted);
        }
    });
});

describe(txVectors.description, function () {
    async function getSigner(ethPrivateKey) {
        const ethWallet = new Wallet(ethPrivateKey);
        const { signer } = await zksync.Signer.fromETHSignature(ethWallet);
        const ethMessageSigner = new zksync.EthMessageSigner(ethWallet, {
            verificationMethod: 'ECDSA',
            isSignedMsgPrefixed: true
        });

        return { signer, ethMessageSigner };
    }

    it('Transfer signature', async function () {
        for (const item of txVectors.items) {
            const { type: txType, ethPrivateKey, data: transferData, ethSignData } = item.inputs;
            const expected = item.outputs;
            const privateKey = parseHexWithPrefix(ethPrivateKey);
            const { signer, ethMessageSigner } = await getSigner(privateKey);

            if (txType === 'Transfer') {
                const signBytes = signer.transferSignBytes(transferData);
                const { signature } = await signer.signSyncTransfer(transferData);

                const { signature: ethSignature } = await ethMessageSigner.ethSignTransfer(ethSignData);
                const ethSignMessage = ethMessageSigner.getTransferEthSignMessage(ethSignData);

                expect(utils.hexlify(signBytes)).to.eql(expected.signBytes, 'Sign bytes do not match');
                expect(signature).to.eql(expected.signature, 'Signature does not match');
                expect(ethSignature).to.eql(expected.ethSignature, 'Ethereum signature does not match');
                expect(utils.hexlify(utils.toUtf8Bytes(ethSignMessage))).to.eql(
                    expected.ethSignMessage,
                    'Ethereum signature message does not match'
                );
            }
        }
    });

    it('Order signature', async function () {
        for (const item of txVectors.items) {
            const { type: txType, ethPrivateKey, data: order, ethSignData } = item.inputs;
            const expected = item.outputs;
            const privateKey = parseHexWithPrefix(ethPrivateKey);
            const { signer, ethMessageSigner } = await getSigner(privateKey);

            if (txType === 'Order') {
                const signBytes = zksync.utils.serializeOrder(order);
                const { signature } = await signer.signSyncOrder(order);

                const { signature: ethSignature } = await ethMessageSigner.ethSignOrder(ethSignData);
                const ethSignMessage = ethMessageSigner.getOrderEthSignMessage(ethSignData);

                expect(utils.hexlify(signBytes)).to.eql(expected.signBytes, 'Sign bytes do not match');
                expect(signature).to.eql(expected.signature, 'Signature does not match');
                expect(ethSignature).to.eql(expected.ethSignature, 'Ethereum signature does not match');
                expect(utils.hexlify(utils.toUtf8Bytes(ethSignMessage))).to.eql(
                    expected.ethSignMessage,
                    'Ethereum signature message does not match'
                );
            }
        }
    });

    it('Swap signature', async function () {
        for (const item of txVectors.items) {
            const { type: txType, ethPrivateKey, data: order, ethSignData } = item.inputs;
            const expected = item.outputs;
            const privateKey = parseHexWithPrefix(ethPrivateKey);
            const { signer, ethMessageSigner } = await getSigner(privateKey);

            if (txType === 'Swap') {
                const signBytes = await zksync.utils.serializeSwap(order);
                const { signature } = await signer.signSyncSwap(order);

                const { signature: ethSignature } = await ethMessageSigner.ethSignSwap(ethSignData);
                const ethSignMessage = ethMessageSigner.getSwapEthSignMessage(ethSignData);

                expect(utils.hexlify(signBytes)).to.eql(expected.signBytes, 'Sign bytes do not match');
                expect(signature).to.eql(expected.signature, 'Signature does not match');
                expect(ethSignature).to.eql(expected.ethSignature, 'Ethereum signature does not match');
                expect(utils.hexlify(utils.toUtf8Bytes(ethSignMessage))).to.eql(
                    expected.ethSignMessage,
                    'Ethereum signature message does not match'
                );
            }
        }
    });

    it('ChangePubKey signature', async function () {
        for (const item of txVectors.items) {
            const { type: txType, ethPrivateKey, data: changePubKeyData, ethSignData } = item.inputs;
            const expected = item.outputs;
            const privateKey = parseHexWithPrefix(ethPrivateKey);
            const { signer, ethMessageSigner } = await getSigner(privateKey);

            if (txType === 'ChangePubKey') {
                const signBytes = signer.changePubKeySignBytes(changePubKeyData);
                const { signature } = await signer.signSyncChangePubKey(changePubKeyData);

                const { signature: ethSignature } = await ethMessageSigner.ethSignChangePubKey(ethSignData);
                const ethSignMessage = ethMessageSigner.getChangePubKeyEthSignMessage(ethSignData);

                expect(utils.hexlify(signBytes)).to.eql(expected.signBytes, 'Sign bytes do not match');
                expect(signature).to.eql(expected.signature, 'Signature does not match');
                expect(ethSignature).to.eql(expected.ethSignature, 'Ethereum signature does not match');
                expect(utils.hexlify(ethSignMessage)).to.eql(
                    expected.ethSignMessage,
                    'Ethereum signature message does not match'
                );
            }
        }
    });

    it('Withdraw signature', async function () {
        for (const item of txVectors.items) {
            const { type: txType, ethPrivateKey, data: withdrawData, ethSignData } = item.inputs;
            const expected = item.outputs;
            const privateKey = parseHexWithPrefix(ethPrivateKey);
            const { signer, ethMessageSigner } = await getSigner(privateKey);

            if (txType === 'Withdraw') {
                const signBytes = signer.withdrawSignBytes(withdrawData);
                const { signature } = await signer.signSyncWithdraw(withdrawData);

                const { signature: ethSignature } = await ethMessageSigner.ethSignWithdraw(ethSignData);
                const ethSignMessage = ethMessageSigner.getWithdrawEthSignMessage(ethSignData);

                expect(utils.hexlify(signBytes)).to.eql(expected.signBytes, 'Sign bytes do not match');
                expect(signature).to.eql(expected.signature, 'Signature does not match');
                expect(ethSignature).to.eql(expected.ethSignature, 'Ethereum signature does not match');
                expect(utils.hexlify(utils.toUtf8Bytes(ethSignMessage))).to.eql(
                    expected.ethSignMessage,
                    'Ethereum signature message does not match'
                );
            }
        }
    });

    it('ForcedExit signature', async function () {
        for (const item of txVectors.items) {
            const { type: txType, ethPrivateKey, data: forcedExit } = item.inputs;
            const expected = item.outputs;
            const privateKey = parseHexWithPrefix(ethPrivateKey);
            const { signer } = await getSigner(privateKey);

            if (txType === 'ForcedExit') {
                const signBytes = signer.forcedExitSignBytes(forcedExit);
                const { signature } = await signer.signSyncForcedExit(forcedExit);

                expect(utils.hexlify(signBytes)).to.eql(expected.signBytes, 'Sign bytes do not match');
                expect(signature).to.eql(expected.signature, 'Signature does not match');
            }
        }
    });

    it('MintNFT signature', async function () {
        for (const item of txVectors.items) {
            const { type: txType, ethPrivateKey, data: mintNFTData, ethSignData } = item.inputs;
            const expected = item.outputs;
            const privateKey = parseHexWithPrefix(ethPrivateKey);
            const { signer, ethMessageSigner } = await getSigner(privateKey);

            if (txType === 'MintNFT') {
                const tx: MintNFT = {
                    ...mintNFTData,
                    type: 'MintNFT',
                    feeToken: mintNFTData.feeTokenId
                };
                const signBytes = await serializeTx(tx);
                const { signature } = await signer.signMintNFT(mintNFTData);

                const { signature: ethSignature } = await ethMessageSigner.ethSignMintNFT(ethSignData);
                const ethSignMessage = ethMessageSigner.getMintNFTEthSignMessage(ethSignData);

                expect(utils.hexlify(signBytes)).to.eql(expected.signBytes, 'Sign bytes do not match');
                expect(signature).to.eql(expected.signature, 'Signature does not match');
                expect(ethSignature).to.eql(expected.ethSignature, 'Ethereum signature does not match');
                expect(utils.hexlify(utils.toUtf8Bytes(ethSignMessage))).to.eql(
                    expected.ethSignMessage,
                    'Ethereum signature message does not match'
                );
            }
        }
    });

    it('WithdrawNFT signature', async function () {
        for (const item of txVectors.items) {
            const { type: txType, ethPrivateKey, data: withdrawNFTData, ethSignData } = item.inputs;
            const expected = item.outputs;
            const privateKey = parseHexWithPrefix(ethPrivateKey);
            const { signer, ethMessageSigner } = await getSigner(privateKey);

            if (txType === 'WithdrawNFT') {
                const tx: WithdrawNFT = {
                    ...withdrawNFTData,
                    type: 'WithdrawNFT',
                    token: withdrawNFTData.tokenId,
                    feeToken: withdrawNFTData.feeTokenId
                };
                const signBytes = await serializeTx(tx);
                const { signature } = await signer.signWithdrawNFT(withdrawNFTData);

                const { signature: ethSignature } = await ethMessageSigner.ethSignWithdrawNFT(ethSignData);
                const ethSignMessage = ethMessageSigner.getWithdrawNFTEthSignMessage(ethSignData);

                expect(utils.hexlify(signBytes)).to.eql(expected.signBytes, 'Sign bytes do not match');
                expect(signature).to.eql(expected.signature, 'Signature does not match');
                expect(ethSignature).to.eql(expected.ethSignature, 'Ethereum signature does not match');
                expect(utils.hexlify(utils.toUtf8Bytes(ethSignMessage))).to.eql(
                    expected.ethSignMessage,
                    'Ethereum signature message does not match'
                );
            }
        }
    });
});

describe(txHashVectors.description, function () {
    it('Transaction hash', async function () {
        for (const item of txHashVectors.items) {
            const tx = item.inputs.tx;
            const expectedHash = item.outputs.hash;
            const hash = await getTxHash(tx);
            expect(hash).to.eql(expectedHash, 'Hash does not match');
        }
    });
});
