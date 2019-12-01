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
import { Address, CloseAccount, Transfer, Withdraw } from "./types";

const MAX_NUMBER_OF_TOKENS = 4096;
const MAX_NUMBER_OF_ACCOUNTS = 1 << 24;

export class Signer {
    readonly privateKey: BN;
    readonly publicKey: curve.edwards.EdwardsPoint;

    private constructor(privKey: BN) {
        this.privateKey = privKey;
        this.publicKey = privateKeyToPublicKey(this.privateKey);
    }

    address(): Address {
        return `0x${pubkeyToAddress(this.publicKey).toString("hex")}`;
    }

    signSyncTransfer(transfer: {
        to: Address;
        tokenId: number;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce: number;
    }): Transfer {
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
            type: "Transfer",
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
    }): Withdraw {
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
            type: "Withdraw",
            account: this.address(),
            ethAddress: withdraw.ethAddress,
            token: withdraw.tokenId,
            amount: utils.bigNumberify(withdraw.amount).toString(),
            fee: utils.bigNumberify(withdraw.fee).toString(),
            nonce: withdraw.nonce,
            signature
        };
    }

    signSyncCloseAccount(close: { nonce: number }): CloseAccount {
        const type = Buffer.from([4]);
        const account = serializeAddress(this.address());
        const nonce = serializeNonce(close.nonce);

        const msg = Buffer.concat([type, account, nonce]);
        const signature = signTransactionBytes(this.privateKey, msg);

        return {
            type: "Close",
            account: this.address(),
            nonce: close.nonce,
            signature
        };
    }

    syncEmergencyWithdrawSignature(emergencyWithdraw: {
        accountId: number;
        ethAddress: string;
        tokenId: number;
        nonce: number;
    }): Buffer {
        const type = Buffer.from([6]);
        const packed_pubkey = serializePointPacked(this.publicKey);
        const account_id = serializeAccountId(emergencyWithdraw.accountId);
        const eth_address = serializeAddress(emergencyWithdraw.ethAddress);
        const token = serializeTokenId(emergencyWithdraw.tokenId);
        const nonce = serializeNonce(emergencyWithdraw.nonce);
        const msg = Buffer.concat([
            type,
            account_id,
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

    static fromPrivateKey(pk: BN): Signer {
        return new Signer(pk);
    }

    static fromSeed(seed: Buffer): Signer {
        return new Signer(privateKeyFromSeed(seed));
    }
}

// Sync or eth address
function serializeAddress(address: Address | string): Buffer {
    const addressBytes = Buffer.from(address.substr(2), "hex");
    if (addressBytes.length != 20) {
        throw new Error("Address should be 20 bytes long");
    }
    return addressBytes;
}

function serializeAccountId(accountId: number): Buffer {
    if (accountId < 0) {
        throw new Error("Negative account id");
    }
    if (accountId >= MAX_NUMBER_OF_ACCOUNTS) {
        throw new Error("AccountId is too big");
    }
    const buffer = Buffer.alloc(4);
    buffer.writeUInt32BE(accountId, 0);
    // only 3 bytes
    return buffer.slice(1);
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
