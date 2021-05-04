import * as ethers from 'ethers';
import { TxEthSignature, EthSignerType, PubKeyHash } from './types';
import { getSignedBytesFromMessage, signMessagePersonalAPI, getChangePubkeyMessage } from './utils';

/**
 * Wrapper around `ethers.Signer` which provides convenient methods to get and sign messages required for zkSync.
 */
export class EthMessageSigner {
    constructor(private ethSigner: ethers.Signer, private ethSignerType?: EthSignerType) {}

    async getEthMessageSignature(message: ethers.utils.BytesLike): Promise<TxEthSignature> {
        if (this.ethSignerType == null) {
            throw new Error('ethSignerType is unknown');
        }

        const signedBytes = getSignedBytesFromMessage(message, !this.ethSignerType.isSignedMsgPrefixed);

        const signature = await signMessagePersonalAPI(this.ethSigner, signedBytes);

        return {
            type: this.ethSignerType.verificationMethod === 'ECDSA' ? 'EthereumSignature' : 'EIP1271Signature',
            signature
        };
    }

    getTransferEthSignMessage(transfer: {
        stringAmount: string;
        stringToken: string;
        stringFee: string;
        to: string;
        nonce: number;
        accountId: number;
    }): string {
        let humanReadableTxInfo = this.getTransferEthMessagePart(transfer);
        if (humanReadableTxInfo.length != 0) {
            humanReadableTxInfo += '\n';
        }
        humanReadableTxInfo += `Nonce: ${transfer.nonce}`;

        return humanReadableTxInfo;
    }

    async ethSignTransfer(transfer: {
        stringAmount: string;
        stringToken: string;
        stringFee: string;
        to: string;
        nonce: number;
        accountId: number;
    }): Promise<TxEthSignature> {
        const message = this.getTransferEthSignMessage(transfer);
        return await this.getEthMessageSignature(message);
    }

    async ethSignForcedExit(forcedExit: {
        stringToken: string;
        stringFee: string;
        target: string;
        nonce: number;
    }): Promise<TxEthSignature> {
        const message = this.getForcedExitEthSignMessage(forcedExit);
        return await this.getEthMessageSignature(message);
    }

    getMintNFTEthMessagePart(mintNFT: {
        stringToken: string;
        stringFee: string;
        recipient: string;
        contentHash: string;
    }): string {
        let humanReadableTxInfo = `MintNFT ${mintNFT.contentHash} for: ${mintNFT.recipient.toLowerCase()}`;

        if (mintNFT.stringFee != null) {
            humanReadableTxInfo += `\nFee: ${mintNFT.stringFee} ${mintNFT.stringToken}`;
        }

        return humanReadableTxInfo;
    }

    getMintNFTEthSignMessage(mintNFT: {
        stringToken: string;
        stringFee: string;
        recipient: string;
        contentHash: string;
        nonce: number;
    }): string {
        let humanReadableTxInfo = this.getMintNFTEthMessagePart(mintNFT);

        humanReadableTxInfo += `\nNonce: ${mintNFT.nonce}`;

        return humanReadableTxInfo;
    }

    getWithdrawNFTEthMessagePart(withdrawNFT: {
        stringToken: string;
        to: string;
        stringFee: string;
        stringFeeToken: string;
    }): string {
        let humanReadableTxInfo = `WithdrawNFT ${withdrawNFT.stringToken} to: ${withdrawNFT.to.toLowerCase()}`;

        if (withdrawNFT.stringFee != null) {
            humanReadableTxInfo += `\nFee: ${withdrawNFT.stringFee} ${withdrawNFT.stringFeeToken}`;
        }

        return humanReadableTxInfo;
    }

    getWithdrawNFTEthSignMessage(withdrawNFT: {
        stringToken: string;
        to: string;
        stringFee: string;
        stringFeeToken: string;
        nonce: number;
    }): string {
        let humanReadableTxInfo = this.getWithdrawNFTEthMessagePart(withdrawNFT);

        humanReadableTxInfo += `\nNonce: ${withdrawNFT.nonce}`;

        return humanReadableTxInfo;
    }

    getWithdrawEthSignMessage(withdraw: {
        stringAmount: string;
        stringToken: string;
        stringFee: string;
        ethAddress: string;
        nonce: number;
        accountId: number;
    }): string {
        let humanReadableTxInfo = this.getWithdrawEthMessagePart(withdraw);
        if (humanReadableTxInfo.length != 0) {
            humanReadableTxInfo += '\n';
        }
        humanReadableTxInfo += `Nonce: ${withdraw.nonce}`;

        return humanReadableTxInfo;
    }

    getForcedExitEthSignMessage(forcedExit: {
        stringToken: string;
        stringFee: string;
        target: string;
        nonce: number;
    }): string {
        let humanReadableTxInfo = this.getForcedExitEthMessagePart(forcedExit);
        humanReadableTxInfo += `\nNonce: ${forcedExit.nonce}`;
        return humanReadableTxInfo;
    }

    getTransferEthMessagePart(tx: {
        stringAmount: string;
        stringToken: string;
        stringFee: string;
        ethAddress?: string;
        to?: string;
    }): string {
        let txType: string, to: string;
        if (tx.ethAddress != undefined) {
            txType = 'Withdraw';
            to = tx.ethAddress;
        } else if (tx.to != undefined) {
            txType = 'Transfer';
            to = tx.to;
        } else {
            throw new Error('Either to or ethAddress field must be present');
        }

        let message = '';
        if (tx.stringAmount != null) {
            message += `${txType} ${tx.stringAmount} ${tx.stringToken} to: ${to.toLowerCase()}`;
        }
        if (tx.stringFee != null) {
            if (message.length != 0) {
                message += '\n';
            }
            message += `Fee: ${tx.stringFee} ${tx.stringToken}`;
        }
        return message;
    }

    getWithdrawEthMessagePart(tx: {
        stringAmount: string;
        stringToken: string;
        stringFee: string;
        ethAddress?: string;
        to?: string;
    }): string {
        return this.getTransferEthMessagePart(tx);
    }

    getChangePubKeyEthMessagePart(changePubKey: {
        pubKeyHash: PubKeyHash;
        stringToken: string;
        stringFee: string;
    }): string {
        let message = '';
        message += `Set signing key: ${changePubKey.pubKeyHash.replace('sync:', '').toLowerCase()}`;
        if (changePubKey.stringFee != null) {
            message += `\nFee: ${changePubKey.stringFee} ${changePubKey.stringToken}`;
        }
        return message;
    }

    getForcedExitEthMessagePart(forcedExit: { stringToken: string; stringFee: string; target: string }): string {
        let message = `ForcedExit ${forcedExit.stringToken} to: ${forcedExit.target.toLowerCase()}`;
        if (forcedExit.stringFee != null) {
            message += `\nFee: ${forcedExit.stringFee} ${forcedExit.stringToken}`;
        }
        return message;
    }

    async ethSignMintNFT(mintNFT: {
        stringToken: string;
        stringFee: string;
        recipient: string;
        contentHash: string;
        nonce: number;
    }): Promise<TxEthSignature> {
        const message = this.getMintNFTEthSignMessage(mintNFT);
        return await this.getEthMessageSignature(message);
    }

    async ethSignWithdrawNFT(withdrawNFT: {
        stringToken: string;
        to: string;
        stringFee: string;
        stringFeeToken: string;
        nonce: number;
    }): Promise<TxEthSignature> {
        const message = this.getWithdrawNFTEthSignMessage(withdrawNFT);
        return await this.getEthMessageSignature(message);
    }

    async ethSignWithdraw(withdraw: {
        stringAmount: string;
        stringToken: string;
        stringFee: string;
        ethAddress: string;
        nonce: number;
        accountId: number;
    }): Promise<TxEthSignature> {
        const message = this.getWithdrawEthSignMessage(withdraw);
        return await this.getEthMessageSignature(message);
    }

    getChangePubKeyEthSignMessage(changePubKey: {
        pubKeyHash: PubKeyHash;
        nonce: number;
        accountId: number;
    }): Uint8Array {
        return getChangePubkeyMessage(changePubKey.pubKeyHash, changePubKey.nonce, changePubKey.accountId);
    }

    async ethSignChangePubKey(changePubKey: {
        pubKeyHash: PubKeyHash;
        nonce: number;
        accountId: number;
    }): Promise<TxEthSignature> {
        const message = this.getChangePubKeyEthSignMessage(changePubKey);
        return await this.getEthMessageSignature(message);
    }
}
