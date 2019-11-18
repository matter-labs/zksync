import { curve } from "elliptic";
import {
    privateKeyFromSeed,
    privateKeyToPublicKey,
    pubkeyToAddress,
    serializePointPacked,
    signTransactionBytes
} from "./crypto";
import { utils } from "ethers";
import { packAmountChecked, packFeeChecked } from "./utils";
import BN = require("bn.js");
import {
    SyncAddress,
    SyncCloseAccount,
    SyncTransfer,
    SyncWithdraw
} from "./types";

const MAX_NUMBER_OF_TOKENS = 4096;

export class SyncSigner {
    readonly privateKey: BN;
    readonly publicKey: curve.edwards.EdwardsPoint;

    private constructor(privKey: BN) {
        this.privateKey = privKey;
        this.publicKey = privateKeyToPublicKey(this.privateKey);
    }

    address(): SyncAddress {
        return `0x${pubkeyToAddress(this.publicKey).toString("hex")}`;
    }

    signSyncTransfer(transfer: {
        to: SyncAddress;
        tokenId: number;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce: number;
    }): SyncTransfer {
        const type = Buffer.from([5]); // tx type
        const from = serializeAddress(this.address());
        const to = serializeAddress(transfer.to);
        const token = serializeTokenId(transfer.tokenId);
        const amount = serializeAmountPacked(transfer.amount);
        const fee = serializeFeePacked(transfer.fee);
        const nonce = serializeNonce(transfer.nonce);
        const msgBytes = Buffer.concat([
            type,
            from,
            to,
            token,
            amount,
            fee,
            nonce
        ]);

        const signature = signTransactionBytes(this.privateKey, msgBytes);

        return {
            from: this.address(),
            to: transfer.to,
            token: transfer.tokenId,
            amount: utils.bigNumberify(transfer.amount).toString(),
            fee: utils.bigNumberify(transfer.fee).toString(),
            nonce: transfer.nonce,
            signature
        };
    }

    signSyncWithdraw(withdraw: {
        ethAddress: string;
        tokenId: number;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce: number;
    }): SyncWithdraw {
        const typeBytes = Buffer.from([3]);
        const accountBytes = serializeAddress(this.address());
        const ethAddressBytes = serializeAddress(withdraw.ethAddress);
        const tokenIdBytes = serializeTokenId(withdraw.tokenId);
        const amountBytes = serializeAmountFull(withdraw.amount);
        const feeBytes = serializeFeePacked(withdraw.fee);
        const nonceBytes = serializeNonce(withdraw.nonce);
        const msgBytes = Buffer.concat([
            typeBytes,
            accountBytes,
            ethAddressBytes,
            tokenIdBytes,
            amountBytes,
            feeBytes,
            nonceBytes
        ]);
        const signature = signTransactionBytes(this.privateKey, msgBytes);
        return {
            account: this.address(),
            ethAddress: withdraw.ethAddress,
            token: withdraw.tokenId,
            amount: utils.bigNumberify(withdraw.amount).toString(),
            fee: utils.bigNumberify(withdraw.fee).toString(),
            nonce: withdraw.nonce,
            signature
        };
    }

    signSyncCloseAccount(close: { nonce: number }): SyncCloseAccount {
        const type = Buffer.from([4]);
        const account = serializeAddress(this.address());
        const nonce = serializeNonce(close.nonce);

        const msg = Buffer.concat([type, account, nonce]);
        const signature = signTransactionBytes(this.privateKey, msg);

        return {
            account: this.address(),
            nonce: close.nonce,
            signature
        };
    }

    signSyncEmergencyWithdraw(fullExit: {
        ethAddress: string;
        tokenId: number;
        nonce: number;
    }): Buffer {
        const type = Buffer.from([6]);
        const packed_pubkey = serializePointPacked(this.publicKey);
        const eth_address = serializeAddress(fullExit.ethAddress);
        const token = serializeTokenId(fullExit.tokenId);
        const nonce = serializeNonce(fullExit.nonce);
        const msg = Buffer.concat([
            type,
            packed_pubkey,
            eth_address,
            token,
            nonce
        ]);
        return Buffer.from(
            signTransactionBytes(this.privateKey, msg).signature,
            "hex"
        );
    }

    static fromPrivateKey(pk: BN): SyncSigner {
        return new SyncSigner(pk);
    }

    static fromSeed(seed: Buffer): SyncSigner {
        return new SyncSigner(privateKeyFromSeed(seed));
    }
}

// Sync or eth address
function serializeAddress(address: SyncAddress | string): Buffer {
    const addressBytes = Buffer.from(address.substr(2), "hex");
    if (addressBytes.length != 20) {
        throw new Error("Address should be 20 bytes long");
    }
    return addressBytes;
}
function serializeTokenId(tokenId: number): Buffer {
    if (tokenId < 0) {
        throw new Error("Negative tokenId");
    }
    if (tokenId >= MAX_NUMBER_OF_TOKENS) {
        throw new Error("TokenId is too big");
    }
    const buffer = Buffer.alloc(2);
    buffer.writeUInt16BE(tokenId, 0);
    return buffer;
}

function serializeAmountPacked(amount: utils.BigNumberish): Buffer {
    const bnAmount = new BN(utils.bigNumberify(amount).toString());
    return packAmountChecked(bnAmount);
}

function serializeAmountFull(amount: utils.BigNumberish): Buffer {
    const bnAmount = new BN(utils.bigNumberify(amount).toString());
    return bnAmount.toArrayLike(Buffer, "be", 16);
}

function serializeFeePacked(fee: utils.BigNumberish): Buffer {
    const bnFee = new BN(utils.bigNumberify(fee).toString());
    return packFeeChecked(bnFee);
}

function serializeNonce(nonce: number): Buffer {
    if (nonce < 0) {
        throw new Error("Negative nonce");
    }
    const buff = Buffer.alloc(4);
    buff.writeUInt32BE(nonce, 0);
    return buff;
}
