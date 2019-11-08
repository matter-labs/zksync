import {curve} from "elliptic";
import {
    privateKeyFromSeed,
    privateKeyToPublicKey,
    pubkeyToAddress,
    serializePointPacked,
    signTransactionBytes
} from "./crypto";
import {utils} from "ethers";
import {packAmountChecked, packFeeChecked} from "./utils";
import BN = require("bn.js");
import {SyncAddress, SyncTransfer} from "./types";

const MAX_NUMBER_OF_TOKENS= 4096;

export class SyncSigner {
    readonly privateKey: BN;
    readonly publicKey: curve.edwards.EdwardsPoint;

    private constructor(privKey: BN) {
        this.privateKey = privKey;
        this.publicKey = privateKeyToPublicKey(this.privateKey);
    }

    address(): SyncAddress {
        return `0x${pubkeyToAddress(this.publicKey)}`
    }

    signTransfer(transfer: {from: SyncAddress, to: SyncAddress, tokenId: number, amount: utils.BigNumberish, fee: utils.BigNumberish, nonce: number}): SyncTransfer {
        const type = Buffer.from([5]); // tx type
        const from = serializeAddress(transfer.from);
        const to = serializeAddress(transfer.to);
        const token = serializeTokenId(transfer.tokenId);
        const amount = serializeAmountPacked(transfer.amount);
        const fee = serializeFeePacked(transfer.fee);
        const nonce = serializeNonce(transfer.nonce);
        const msg = Buffer.concat([type, from, to, token, amount, fee, nonce]);
        const signature = signTransactionBytes(this.privateKey, msg);
    }

    signWithdraw(ethAddress: string, tokenId: number, amount: utils.BigNumberish, fee: utils.BigNumberish, nonce: number) {
        // serialize message
        const typeBytes = Buffer.from([3]);
        const accountBytes = serializeAddress(this.address());
        const ethAddressBytes = serializeAddress(ethAddress);
        const tokenIdBytes = serializeTokenId(tokenId);
        const amountBytes = serializeAmountFull(amount);
        const feeBytes = serializeFeePacked(fee);
        const nonceBytes = serializeNonce(nonce);
        const msgBytes = Buffer.concat([
            typeBytes,
            accountBytes,
            ethAddressBytes,
            tokenIdBytes,
            amountBytes,
            feeBytes,
            nonceBytes
        ]);

        let signature =

        return signTransactionBytes(this.privateKey, msg);
    }

    signClose(tx: CloseTx) {
        const type = Buffer.from([4]);
        const account = serializeAddress(tx.account);
        const nonce = serializeNonce(tx.nonce);

        const msg = Buffer.concat([type, account, nonce]);
        return signTransactionBytes(this.privateKey, msg);
    }

    signFullExit(op: FullExitReq) {
        const type = Buffer.from([6]);
        const packed_pubkey = serializePointPacked(this.publicKey);
        const eth_address = serializeAddress(op.eth_address);
        const token = serializeTokenId(op.token);
        const nonce = serializeNonce(op.nonce);
        const msg = Buffer.concat([type, packed_pubkey, eth_address, token, nonce]);
        return Buffer.from(signTransactionBytes(this.privateKey, msg).sign, "hex");
    }

    static fromPrivateKey(pk: BN): WalletKeys {
        return new WalletKeys(pk);
    }

    static fromSeed(seed: Buffer) : WalletKeys {
        return new WalletKeys(privateKeyFromSeed(seed));
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
        throw new Error("Negative tokenId")
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
