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
        let humanReadableTxInfo = '';
        if (transfer.stringAmount != null) {
            humanReadableTxInfo += `Transfer ${transfer.stringAmount} ${
                transfer.stringToken
            } to: ${transfer.to.toLowerCase()}\n`;
        }
        if (transfer.stringFee != null) {
            humanReadableTxInfo += `Fee: ${transfer.stringFee} ${transfer.stringToken}\n`;
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

    getWithdrawEthSignMessage(withdraw: {
        stringAmount: string;
        stringToken: string;
        stringFee: string;
        ethAddress: string;
        nonce: number;
        accountId: number;
    }): string {
        let humanReadableTxInfo = '';
        if (withdraw.stringAmount != null) {
            humanReadableTxInfo += `Withdraw ${withdraw.stringAmount} ${
                withdraw.stringToken
            } to: ${withdraw.ethAddress.toLowerCase()}\n`;
        }
        if (withdraw.stringFee != null) {
            humanReadableTxInfo += `Fee: ${withdraw.stringFee} ${withdraw.stringToken}\n`;
        }
        humanReadableTxInfo += `Nonce: ${withdraw.nonce}`;

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
