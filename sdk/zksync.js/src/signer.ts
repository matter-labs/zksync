import { privateKeyFromSeed, signTransactionBytes, privateKeyToPubKeyHash } from './crypto';
import { BigNumber, BigNumberish, ethers } from 'ethers';
import {
    getEthSignatureType,
    signMessagePersonalAPI,
    getSignedBytesFromMessage,
    serializeAccountId,
    serializeAddress,
    serializeTokenId,
    serializeAmountPacked,
    serializeFeePacked,
    serializeNonce,
    serializeAmountFull
} from './utils';
import { Address, EthSignerType, PubKeyHash, Transfer, Withdraw, ForcedExit, ChangePubKey } from './types';

export class Signer {
    readonly privateKey: Uint8Array;

    private constructor(privKey: Uint8Array) {
        this.privateKey = privKey;
    }

    pubKeyHash(): PubKeyHash {
        return privateKeyToPubKeyHash(this.privateKey);
    }

    signSyncTransfer(transfer: {
        accountId: number;
        from: Address;
        to: Address;
        tokenId: number;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
    }): Transfer {
        const type = new Uint8Array([5]); // tx type
        const accountId = serializeAccountId(transfer.accountId);
        const from = serializeAddress(transfer.from);
        const to = serializeAddress(transfer.to);
        const token = serializeTokenId(transfer.tokenId);
        const amount = serializeAmountPacked(transfer.amount);
        const fee = serializeFeePacked(transfer.fee);
        const nonce = serializeNonce(transfer.nonce);
        const msgBytes = ethers.utils.concat([type, accountId, from, to, token, amount, fee, nonce]);

        const signature = signTransactionBytes(this.privateKey, msgBytes);

        return {
            type: 'Transfer',
            accountId: transfer.accountId,
            from: transfer.from,
            to: transfer.to,
            token: transfer.tokenId,
            amount: BigNumber.from(transfer.amount).toString(),
            fee: BigNumber.from(transfer.fee).toString(),
            nonce: transfer.nonce,
            signature
        };
    }

    signSyncWithdraw(withdraw: {
        accountId: number;
        from: Address;
        ethAddress: string;
        tokenId: number;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
    }): Withdraw {
        const typeBytes = new Uint8Array([3]);
        const accountId = serializeAccountId(withdraw.accountId);
        const accountBytes = serializeAddress(withdraw.from);
        const ethAddressBytes = serializeAddress(withdraw.ethAddress);
        const tokenIdBytes = serializeTokenId(withdraw.tokenId);
        const amountBytes = serializeAmountFull(withdraw.amount);
        const feeBytes = serializeFeePacked(withdraw.fee);
        const nonceBytes = serializeNonce(withdraw.nonce);
        const msgBytes = ethers.utils.concat([
            typeBytes,
            accountId,
            accountBytes,
            ethAddressBytes,
            tokenIdBytes,
            amountBytes,
            feeBytes,
            nonceBytes
        ]);
        const signature = signTransactionBytes(this.privateKey, msgBytes);
        return {
            type: 'Withdraw',
            accountId: withdraw.accountId,
            from: withdraw.from,
            to: withdraw.ethAddress,
            token: withdraw.tokenId,
            amount: BigNumber.from(withdraw.amount).toString(),
            fee: BigNumber.from(withdraw.fee).toString(),
            nonce: withdraw.nonce,
            signature
        };
    }

    signSyncForcedExit(forcedExit: {
        initiatorAccountId: number;
        target: Address;
        tokenId: number;
        fee: BigNumberish;
        nonce: number;
    }): ForcedExit {
        const typeBytes = new Uint8Array([8]);
        const initiatorAccountIdBytes = serializeAccountId(forcedExit.initiatorAccountId);
        const targetBytes = serializeAddress(forcedExit.target);
        const tokenIdBytes = serializeTokenId(forcedExit.tokenId);
        const feeBytes = serializeFeePacked(forcedExit.fee);
        const nonceBytes = serializeNonce(forcedExit.nonce);
        const msgBytes = ethers.utils.concat([
            typeBytes,
            initiatorAccountIdBytes,
            targetBytes,
            tokenIdBytes,
            feeBytes,
            nonceBytes
        ]);
        const signature = signTransactionBytes(this.privateKey, msgBytes);
        return {
            type: 'ForcedExit',
            initiatorAccountId: forcedExit.initiatorAccountId,
            target: forcedExit.target,
            token: forcedExit.tokenId,
            fee: BigNumber.from(forcedExit.fee).toString(),
            nonce: forcedExit.nonce,
            signature
        };
    }

    signSyncChangePubKey(changePubKey: {
        accountId: number;
        account: Address;
        newPkHash: PubKeyHash;
        feeTokenId: number;
        fee: BigNumberish;
        nonce: number;
    }): ChangePubKey {
        const typeBytes = new Uint8Array([7]); // Tx type (1 byte)
        const accountIdBytes = serializeAccountId(changePubKey.accountId);
        const accountBytes = serializeAddress(changePubKey.account);
        const pubKeyHashBytes = serializeAddress(changePubKey.newPkHash);
        const tokenIdBytes = serializeTokenId(changePubKey.feeTokenId);
        const feeBytes = serializeFeePacked(changePubKey.fee);
        const nonceBytes = serializeNonce(changePubKey.nonce);
        const msgBytes = ethers.utils.concat([
            typeBytes,
            accountIdBytes,
            accountBytes,
            pubKeyHashBytes,
            tokenIdBytes,
            feeBytes,
            nonceBytes
        ]);
        const signature = signTransactionBytes(this.privateKey, msgBytes);
        return {
            type: 'ChangePubKey',
            accountId: changePubKey.accountId,
            account: changePubKey.account,
            newPkHash: changePubKey.newPkHash,
            feeToken: changePubKey.feeTokenId,
            fee: BigNumber.from(changePubKey.fee).toString(),
            nonce: changePubKey.nonce,
            signature,
            ethSignature: null
        };
    }

    static fromPrivateKey(pk: Uint8Array): Signer {
        return new Signer(pk);
    }

    static fromSeed(seed: Uint8Array): Signer {
        return new Signer(privateKeyFromSeed(seed));
    }

    static async fromETHSignature(
        ethSigner: ethers.Signer
    ): Promise<{
        signer: Signer;
        ethSignatureType: EthSignerType;
    }> {
        let chainID = 1;
        if (ethSigner.provider) {
            const network = await ethSigner.provider.getNetwork();
            chainID = network.chainId;
        }
        let message = 'Access zkSync account.\n\nOnly sign this message for a trusted client!';
        if (chainID !== 1) {
            message += `\nChain ID: ${chainID}.`;
        }
        const signedBytes = getSignedBytesFromMessage(message, false);
        const signature = await signMessagePersonalAPI(ethSigner, signedBytes);
        const address = await ethSigner.getAddress();
        const ethSignatureType = await getEthSignatureType(ethSigner.provider, message, signature, address);
        const seed = ethers.utils.arrayify(signature);
        const signer = Signer.fromSeed(seed);
        return { signer, ethSignatureType };
    }
}
