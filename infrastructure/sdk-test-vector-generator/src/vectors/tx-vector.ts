import { utils } from 'ethers';
import * as ethers from 'ethers';
import * as zksync from 'zksync';
import { TestVector, TestVectorEntry } from '../types';
import { generateArray } from '../utils';
import { MAX_TIMESTAMP } from 'zksync/build/utils';
import { ChangePubKeyOnchain } from 'zksync/build/types';

/**
 * Interface for the transactions test vector.
 */
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
        ethSignData: any | null;
    };
    outputs: {
        // Encoded transaction bytes to be used for signing.
        signBytes: string;
        // Transaction zkSync signature.
        signature: zksync.types.Signature;
        // Message to be used to provie Ethereum signature. `null` if `inputs.ethSignData` is `null`.
        ethSignMessage: string | null;
        // Ethereum signature for a transaction. `null` if `inputs.ethSignData` is `null`.
        ethSignature: string | null;
    };
}

/**
 * Returns the test vector containing the transaction input data and the outputs: encoded transaction bytes,
 * message for Ethereum signature, and both zkSync and Ethereum signatures.
 * All the byte array data fields are represented in a hexadecimal form.
 */
export async function generateTxEncodingVectors(): Promise<TestVector<TxTestEntry>> {
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
    const ethWallet = new ethers.Wallet(ethPrivateKey);
    const fromAddress = ethWallet.address;

    const transferData = {
        accountId: 44,
        from: fromAddress,
        to: '0x19aa2ed8712072e918632259780e587698ef58df',
        tokenId: 0,
        amount: '1000000000000',
        fee: '1000000',
        nonce: 12,
        validFrom: 0,
        validUntil: MAX_TIMESTAMP
    };
    const transferSignBytes = signer.transferSignBytes(transferData);
    const transferSignature = (await signer.signSyncTransfer(transferData)).signature;
    const transferEthSignInput = {
        stringAmount: '1000000000000.0',
        stringToken: 'ETH',
        stringFee: '1000000.0',
        to: transferData.to,
        accountId: transferData.accountId,
        nonce: transferData.nonce
    };
    const transferEthSignMessage = utils.hexlify(
        utils.toUtf8Bytes(ethMessageSigner.getTransferEthSignMessage(transferEthSignInput))
    );
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
    const ethWallet = new ethers.Wallet(ethPrivateKey);
    const fromAddress = ethWallet.address;

    const changePubKeyData = {
        accountId: 55,
        account: fromAddress,
        newPkHash: await signer.pubKeyHash(),
        feeTokenId: 0,
        fee: '1000000000',
        nonce: 13,
        validFrom: 0,
        validUntil: MAX_TIMESTAMP,
        ethAuthData: {
            type: 'Onchain' // this does not matter for L2 signature verification
        } as ChangePubKeyOnchain
    };
    const changePubKeySignBytes = signer.changePubKeySignBytes(changePubKeyData);
    const changePubKeySignature = (await signer.signSyncChangePubKey(changePubKeyData)).signature;
    const changePubKeyEthSignInput = {
        pubKeyHash: changePubKeyData.newPkHash,
        accountId: changePubKeyData.accountId,
        nonce: changePubKeyData.nonce
    };
    const changePubKeyEthSignMessage = utils.hexlify(
        ethMessageSigner.getChangePubKeyEthSignMessage(changePubKeyEthSignInput)
    );
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
    const ethWallet = new ethers.Wallet(ethPrivateKey);
    const fromAddress = ethWallet.address;

    const withdrawData = {
        accountId: 44,
        from: fromAddress,
        ethAddress: '0x19aa2ed8712072e918632259780e587698ef58df',
        tokenId: 0,
        amount: '1000000000000',
        fee: '1000000',
        nonce: 12,
        validFrom: 0,
        validUntil: MAX_TIMESTAMP
    };
    const withdrawSignBytes = signer.withdrawSignBytes(withdrawData);
    const withdrawSignature = (await signer.signSyncWithdraw(withdrawData)).signature;
    const withdrawEthSignInput = {
        stringAmount: '1000000000000.0',
        stringToken: 'ETH',
        stringFee: '1000000.0',
        ethAddress: withdrawData.ethAddress,
        accountId: withdrawData.accountId,
        nonce: withdrawData.nonce
    };
    const withdrawEthSignMessage = utils.hexlify(
        utils.toUtf8Bytes(ethMessageSigner.getWithdrawEthSignMessage(withdrawEthSignInput))
    );
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
    const ethWallet = new ethers.Wallet(ethPrivateKey);
    const fromAddress = ethWallet.address;

    const forcedExitData = {
        initiatorAccountId: 44,
        from: fromAddress,
        target: '0x19aa2ed8712072e918632259780e587698ef58df',
        tokenId: 0,
        fee: '1000000',
        nonce: 12,
        validFrom: 0,
        validUntil: MAX_TIMESTAMP
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
