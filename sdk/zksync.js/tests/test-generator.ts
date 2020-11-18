// Generator for test vectors to be used by various SDK

import { utils } from 'ethers';
import * as ethers from 'ethers';
import * as zksync from 'zksync';

export interface TestVectorEntry {
    inputs: any;
    outputs: any;
}

export interface TestVector {
    description: string;
    items: TestVectorEntry[];
}

export async function generateSDKTestVectors(outputFile: string = 'test_vectors.json') {
    const cryptoVectors = await generateCryptoTestVectors();
    const txVectors = await generateTxEncodingVectors();
}

/**
 * Generates an filled data array.
 */
function generateArray(length: number): Uint8Array {
    const data = new Uint8Array(length);
    for (let i = 0; i < length; i++) {
        data[i] = i % 255;
    }

    return data;
}

/**
 * Returns the test vector to generate cryptographic primitives.
 * All the data fields are represented in a hexadecimal form.
 *
 * Output format:
 *
 * ```js
 * {
 *   "description": string, // Description of test vector
 *   {
 *     "inputs": {
 *       "seed": string, // Seed to generate private key.
 *       "message": string // M>essage to be signed.
 *     },
 *     "outputs": {
 *        "privateKey": string, // Private key to be obtained from seed.
 *        "pubKeyHash": string, // Hash of a public key corresponding to the generated private key.
 *        "signature": string // Signature obtained using private key and message.
 *     }
 *   }[]
 * }
 * ```
 */
async function generateCryptoTestVectors(): Promise<TestVector> {
    const seed = generateArray(32);
    const bytesToSign = generateArray(64);

    const privateKey = await zksync.crypto.privateKeyFromSeed(seed);
    const { pubKey, signature } = await zksync.crypto.signTransactionBytes(privateKey, bytesToSign);

    const item = {
        inputs: {
            seed: utils.hexlify(seed),
            message: utils.hexlify(bytesToSign)
        },
        outputs: {
            privateKey: utils.hexlify(privateKey),
            pubKeyhash: utils.hexlify(pubKey),
            signature: utils.hexlify(signature)
        }
    };

    return {
        description: 'Contains the seed for private key and the message for signing',
        items: [item]
    };
}

async function generateTxEncodingVectors(): Promise<TestVector> {
    const ethPrivateKey = generateArray(32);
    const ethSigner = new ethers.Wallet(ethPrivateKey);
    const { signer } = await zksync.Signer.fromETHSignature(ethSigner);
    const ethMessageSigner = new zksync.EthMessageSigner(ethSigner);

    const transferItem = await getTransferSignVector(ethPrivateKey, signer, ethMessageSigner);
    const changePubKeyItem = await getChangePubKeySignVector(ethPrivateKey, signer, ethMessageSigner);
    const withdrawItem = await getWithdrawSignVector(ethPrivateKey, signer, ethMessageSigner);
    const forcedExitItem = await getForcedExitSignVector(ethPrivateKey, signer);

    const items = [transferItem, changePubKeyItem, withdrawItem, forcedExitItem];

    return {
        description: 'Contains various zkSync transactions as inputs and zkSync and Ethereum signature data as outputs',
        items: items
    };
}

async function getTransferSignVector(
    ethPrivateKey: Uint8Array,
    signer: zksync.Signer,
    ethMessageSigner: zksync.EthMessageSigner
): Promise<TestVectorEntry> {
    const transferData = {
        accountId: 44,
        from: '0xcdb6aaa2607df186f7dd2d8eb4ee60f83720b045',
        to: '0x19aa2ed8712072e918632259780e587698ef58df',
        tokenId: 0,
        amount: '0.1',
        fee: '0.001',
        nonce: 12
    };
    const transferSignBytes = signer.transferSignBytes(transferData);
    const transferSignature = (await signer.signSyncTransfer(transferData)).signature;
    const transferEthSignInput = {
        stringAmount: '0.1',
        stringToken: 'ETH',
        stringFee: '0.001',
        to: transferData.to,
        accountId: transferData.accountId,
        nonce: transferData.nonce
    };
    const transferEthSignMessage = ethMessageSigner.getTransferEthSignMessage(transferEthSignInput);
    const transferEthSignature = await ethMessageSigner.ethSignTransfer(transferEthSignInput);

    const transferItem = {
        inputs: {
            type: 'transfer',
            ethPrivateKey: utils.hexlify(ethPrivateKey),
            data: transferData,
            ethSignData: transferEthSignInput
        },
        outputs: {
            signBytes: transferSignBytes,
            signature: transferSignature,
            ethSignMessage: transferEthSignMessage,
            ethSignature: transferEthSignature.signature
        }
    };

    return transferItem;
}

async function getChangePubKeySignVector(
    ethPrivateKey: Uint8Array,
    signer: zksync.Signer,
    ethMessageSigner: zksync.EthMessageSigner
): Promise<TestVectorEntry> {
    const changePubKeyData = {
        accountId: 55,
        account: '0xcdb6aaa2607df186f7dd2d8eb4ee60f83720b045',
        newPkHash: await signer.pubKeyHash(),
        feeTokenId: 0,
        fee: '0.01',
        nonce: 13
    };
    const changePubKeySignBytes = signer.changePubKeySignBytes(changePubKeyData);
    const changePubKeySignature = (await signer.signSyncChangePubKey(changePubKeyData)).signature;
    const changePubKeyEthSignInput = {
        pubKeyHash: changePubKeyData.newPkHash,
        accountId: changePubKeyData.accountId,
        nonce: changePubKeyData.nonce
    };
    const changePubKeyEthSignMessage = ethMessageSigner.getChangePubKeyEthSignMessage(changePubKeyEthSignInput);
    const changePubKeyEthSignature = await ethMessageSigner.ethSignChangePubKey(changePubKeyEthSignInput);

    const changePubKeyItem = {
        inputs: {
            type: 'changePubKey',
            ethPrivateKey: utils.hexlify(ethPrivateKey),
            data: changePubKeyData,
            ethSignData: changePubKeyEthSignInput
        },
        outputs: {
            signBytes: changePubKeySignBytes,
            signature: changePubKeySignature,
            ethSignMessage: changePubKeyEthSignMessage,
            ethSignature: changePubKeyEthSignature.signature
        }
    };

    return changePubKeyItem;
}

async function getWithdrawSignVector(
    ethPrivateKey: Uint8Array,
    signer: zksync.Signer,
    ethMessageSigner: zksync.EthMessageSigner
): Promise<TestVectorEntry> {
    const withdrawData = {
        accountId: 44,
        from: '0xcdb6aaa2607df186f7dd2d8eb4ee60f83720b045',
        ethAddress: '0x19aa2ed8712072e918632259780e587698ef58df',
        tokenId: 0,
        amount: '0.1',
        fee: '0.001',
        nonce: 12
    };
    const withdrawSignBytes = signer.withdrawSignBytes(withdrawData);
    const withdrawSignature = (await signer.signSyncWithdraw(withdrawData)).signature;
    const withdrawEthSignInput = {
        stringAmount: '0.1',
        stringToken: 'ETH',
        stringFee: '0.001',
        ethAddress: withdrawData.ethAddress,
        accountId: withdrawData.accountId,
        nonce: withdrawData.nonce
    };
    const withdrawEthSignMessage = ethMessageSigner.getWithdrawEthSignMessage(withdrawEthSignInput);
    const withdrawEthSignature = await ethMessageSigner.ethSignWithdraw(withdrawEthSignInput);

    const withdrawItem = {
        inputs: {
            type: 'withdraw',
            ethPrivateKey: utils.hexlify(ethPrivateKey),
            data: withdrawData,
            ethSignData: withdrawEthSignInput
        },
        outputs: {
            signBytes: withdrawSignBytes,
            signature: withdrawSignature,
            ethSignMessage: withdrawEthSignMessage,
            ethSignature: withdrawEthSignature.signature
        }
    };

    return withdrawItem;
}

async function getForcedExitSignVector(ethPrivateKey: Uint8Array, signer: zksync.Signer): Promise<TestVectorEntry> {
    const forcedExitData = {
        initiatorAccountId: 44,
        from: '0xcdb6aaa2607df186f7dd2d8eb4ee60f83720b045',
        target: '0x19aa2ed8712072e918632259780e587698ef58df',
        tokenId: 0,
        fee: '0.001',
        nonce: 12
    };
    const forcedExitSignBytes = signer.forcedExitSignBytes(forcedExitData);
    const forcedExitSignature = (await signer.signSyncForcedExit(forcedExitData)).signature;

    const forcedExitItem = {
        inputs: {
            type: 'forcedExit',
            ethPrivateKey: utils.hexlify(ethPrivateKey),
            data: forcedExitData,
            ethSignData: null
        },
        outputs: {
            signBytes: forcedExitSignBytes,
            signature: forcedExitSignature,
            ethSignMessage: null,
            ethSignature: null
        }
    };

    return forcedExitItem;
}
