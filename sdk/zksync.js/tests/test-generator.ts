// Generator for test vectors to be used by various SDK

import { utils } from 'ethers';
import * as ethers from 'ethers';
import * as zksync from 'zksync';
import * as fs from 'fs';

export interface TestVectorEntry {
    inputs: any;
    outputs: any;
}

export interface CryptoPrimitivesTestEntry extends TestVectorEntry {
    inputs: {
        // Seed to generate private key.
        seed: string;
        // Message to be signed.
        message: string;
    };
    outputs: {
        // Private key to be obtained from seed.
        privateKey: string;
        // Hash of a public key corresponding to the generated private key.
        pubKeyHash: string;
        // Signature obtained using private key and message.
        signature: string;
    };
}

export interface TxTestEntry extends TestVectorEntry {
    inputs: {
        // Type of transaction. Valid values are: `Transfer`, `Withdraw`, `ChangePubKey`, `ForcedExit`.
        type: string;
        // Ethereum private key. zkSync private key should be derived from it.
        ethPrivateKey: string;
        // Transaction-specific input.
        data: any;
        // Transactin-specific input to generate Ethereum signature.
        // Can be `null` if Ethereum signature is not required for transaction
        ethSignData?: any;
    };
    outputs: {
        // Encoded transaction bytes to be used for signing.
        signBytes: string;
        // Transaction zkSync signature.
        signature: zksync.types.Signature;
        // Message to be used to provie Ethereum signature. `null` if `inputs.ethSignData` is `null`.
        ethSignMessage?: string;
        // Ethereum signature for a transaction. `null` if `inputs.ethSignData` is `null`.
        ethSignature?: string;
    };
}

export interface TestVector<T> {
    description: string;
    items: T[];
}

export async function generateSDKTestVectors(outputFile: string = 'test_vectors.json') {
    const cryptoVectors = await generateCryptoTestVectors();
    const txVectors = await generateTxEncodingVectors();

    const resultTestVector = {
        cryptoPrimitivesTest: cryptoVectors,
        txTest: txVectors
    };

    const testVectorJSON = JSON.stringify(resultTestVector, null, 2);

    fs.writeFileSync(outputFile, testVectorJSON);
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
 */
async function generateCryptoTestVectors(): Promise<TestVector<CryptoPrimitivesTestEntry>> {
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
            pubKeyHash: pubKey,
            signature: signature
        }
    };

    return {
        description: 'Contains the seed for private key and the message for signing',
        items: [item]
    };
}

/**
 * Returns the test vector containing the transaction input data and the outputs: encoded transaction bytes,
 * message for Ethereum signature, and both zkSync and Ethereum signatures.
 * All the byte array data fields are represented in a hexadecimal form.
 */
async function generateTxEncodingVectors(): Promise<TestVector<TxTestEntry>> {
    const ethPrivateKey = generateArray(32);
    const ethSigner = new ethers.Wallet(ethPrivateKey);
    const { signer } = await zksync.Signer.fromETHSignature(ethSigner);
    const ethMessageSigner = new zksync.EthMessageSigner(ethSigner, {
        verificationMethod: 'ECDSA',
        isSignedMsgPrefixed: true
    });

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
): Promise<TxTestEntry> {
    const transferData = {
        accountId: 44,
        from: '0xcdb6aaa2607df186f7dd2d8eb4ee60f83720b045',
        to: '0x19aa2ed8712072e918632259780e587698ef58df',
        tokenId: 0,
        amount: '1000000000000',
        fee: '1000000',
        nonce: 12
    };
    const transferSignBytes = signer.transferSignBytes(transferData);
    const transferSignature = (await signer.signSyncTransfer(transferData)).signature;
    const transferEthSignInput = {
        stringAmount: '1000000000000',
        stringToken: 'ETH',
        stringFee: '1000000',
        to: transferData.to,
        accountId: transferData.accountId,
        nonce: transferData.nonce
    };
    const transferEthSignMessage = ethMessageSigner.getTransferEthSignMessage(transferEthSignInput);
    const transferEthSignature = await ethMessageSigner.ethSignTransfer(transferEthSignInput);

    const transferItem = {
        inputs: {
            type: 'Transfer',
            ethPrivateKey: utils.hexlify(ethPrivateKey),
            data: transferData,
            ethSignData: transferEthSignInput
        },
        outputs: {
            signBytes: utils.hexlify(transferSignBytes),
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
): Promise<TxTestEntry> {
    const changePubKeyData = {
        accountId: 55,
        account: '0xcdb6aaa2607df186f7dd2d8eb4ee60f83720b045',
        newPkHash: await signer.pubKeyHash(),
        feeTokenId: 0,
        fee: '1000000000',
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
            type: 'ChangePubKey',
            ethPrivateKey: utils.hexlify(ethPrivateKey),
            data: changePubKeyData,
            ethSignData: changePubKeyEthSignInput
        },
        outputs: {
            signBytes: utils.hexlify(changePubKeySignBytes),
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
): Promise<TxTestEntry> {
    const withdrawData = {
        accountId: 44,
        from: '0xcdb6aaa2607df186f7dd2d8eb4ee60f83720b045',
        ethAddress: '0x19aa2ed8712072e918632259780e587698ef58df',
        tokenId: 0,
        amount: '1000000000000',
        fee: '1000000',
        nonce: 12
    };
    const withdrawSignBytes = signer.withdrawSignBytes(withdrawData);
    const withdrawSignature = (await signer.signSyncWithdraw(withdrawData)).signature;
    const withdrawEthSignInput = {
        stringAmount: '1000000000000',
        stringToken: 'ETH',
        stringFee: '1000000',
        ethAddress: withdrawData.ethAddress,
        accountId: withdrawData.accountId,
        nonce: withdrawData.nonce
    };
    const withdrawEthSignMessage = ethMessageSigner.getWithdrawEthSignMessage(withdrawEthSignInput);
    const withdrawEthSignature = await ethMessageSigner.ethSignWithdraw(withdrawEthSignInput);

    const withdrawItem = {
        inputs: {
            type: 'Withdraw',
            ethPrivateKey: utils.hexlify(ethPrivateKey),
            data: withdrawData,
            ethSignData: withdrawEthSignInput
        },
        outputs: {
            signBytes: utils.hexlify(withdrawSignBytes),
            signature: withdrawSignature,
            ethSignMessage: withdrawEthSignMessage,
            ethSignature: withdrawEthSignature.signature
        }
    };

    return withdrawItem;
}

async function getForcedExitSignVector(ethPrivateKey: Uint8Array, signer: zksync.Signer): Promise<TxTestEntry> {
    const forcedExitData = {
        initiatorAccountId: 44,
        from: '0xcdb6aaa2607df186f7dd2d8eb4ee60f83720b045',
        target: '0x19aa2ed8712072e918632259780e587698ef58df',
        tokenId: 0,
        fee: '1000000',
        nonce: 12
    };
    const forcedExitSignBytes = signer.forcedExitSignBytes(forcedExitData);
    const forcedExitSignature = (await signer.signSyncForcedExit(forcedExitData)).signature;

    const forcedExitItem = {
        inputs: {
            type: 'ForcedExit',
            ethPrivateKey: utils.hexlify(ethPrivateKey),
            data: forcedExitData,
            ethSignData: null
        },
        outputs: {
            signBytes: utils.hexlify(forcedExitSignBytes),
            signature: forcedExitSignature,
            ethSignMessage: null,
            ethSignature: null
        }
    };

    return forcedExitItem;
}

(async () => {
    await generateSDKTestVectors();
})();
