import { privateKeyFromSeed, signTransactionBytes, privateKeyToPubKeyHash } from "./crypto";
import { BigNumber, BigNumberish, ethers } from "ethers";
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
} from "./utils";
import { Address, EthSignerType, PubKeyHash, Transfer, Withdraw } from "./types";

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
            type: "Transfer",
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
            type: "Withdraw",
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
        const message = "Access zkSync account.\n" + "\n" + "Only sign this message for a trusted client!";
        const signedBytes = getSignedBytesFromMessage(message, false);
        const signature = await signMessagePersonalAPI(ethSigner, signedBytes);
        const address = await ethSigner.getAddress();
        const ethSignatureType = await getEthSignatureType(ethSigner.provider, message, signature, address);
        const seed = ethers.utils.arrayify(signature);
        const signer = Signer.fromSeed(seed);
        return { signer, ethSignatureType };
    }
}
