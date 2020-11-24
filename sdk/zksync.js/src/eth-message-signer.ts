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
        const humanReadableTxInfo =
            `Transfer ${transfer.stringAmount} ${transfer.stringToken}\n` +
            `To: ${transfer.to.toLowerCase()}\n` +
            `Nonce: ${transfer.nonce}\n` +
            `Fee: ${transfer.stringFee} ${transfer.stringToken}\n` +
            `Account Id: ${transfer.accountId}`;

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

    getWithdrawEthSignMessage(withdraw: {
        stringAmount: string;
        stringToken: string;
        stringFee: string;
        ethAddress: string;
        nonce: number;
        accountId: number;
    }): string {
        const humanReadableTxInfo =
            `Withdraw ${withdraw.stringAmount} ${withdraw.stringToken}\n` +
            `To: ${withdraw.ethAddress.toLowerCase()}\n` +
            `Nonce: ${withdraw.nonce}\n` +
            `Fee: ${withdraw.stringFee} ${withdraw.stringToken}\n` +
            `Account Id: ${withdraw.accountId}`;

        return humanReadableTxInfo;
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

    getChangePubKeyEthSignMessage(changePubKey: { pubKeyHash: PubKeyHash; nonce: number; accountId: number }): string {
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
