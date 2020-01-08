"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
var crypto_1 = require("./crypto");
var ethers_1 = require("ethers");
var utils_1 = require("./utils");
var BN = require("bn.js");
var MAX_NUMBER_OF_TOKENS = 4096;
var MAX_NUMBER_OF_ACCOUNTS = 1 << 24;
var Signer = /** @class */ (function () {
    function Signer(privKey) {
        this.privateKey = privKey;
        this.publicKey = crypto_1.privateKeyToPublicKey(this.privateKey);
    }
    Signer.prototype.address = function () {
        return "sync:" + crypto_1.pubkeyToAddress(this.publicKey).toString("hex");
    };
    Signer.prototype.signSyncTransfer = function (transfer) {
        var type = Buffer.from([5]); // tx type
        var from = serializeAddress(this.address());
        var to = serializeAddress(transfer.to);
        var token = serializeTokenId(transfer.tokenId);
        var amount = serializeAmountPacked(transfer.amount);
        var fee = serializeFeePacked(transfer.fee);
        var nonce = serializeNonce(transfer.nonce);
        var msgBytes = Buffer.concat([
            type,
            from,
            to,
            token,
            amount,
            fee,
            nonce
        ]);
        var signature = crypto_1.signTransactionBytes(this.privateKey, msgBytes);
        return {
            type: "Transfer",
            from: this.address(),
            to: transfer.to,
            token: transfer.tokenId,
            amount: ethers_1.utils.bigNumberify(transfer.amount).toString(),
            fee: ethers_1.utils.bigNumberify(transfer.fee).toString(),
            nonce: transfer.nonce,
            signature: signature
        };
    };
    Signer.prototype.signSyncWithdraw = function (withdraw) {
        var typeBytes = Buffer.from([3]);
        var accountBytes = serializeAddress(this.address());
        var ethAddressBytes = serializeAddress(withdraw.ethAddress);
        var tokenIdBytes = serializeTokenId(withdraw.tokenId);
        var amountBytes = serializeAmountFull(withdraw.amount);
        var feeBytes = serializeFeePacked(withdraw.fee);
        var nonceBytes = serializeNonce(withdraw.nonce);
        var msgBytes = Buffer.concat([
            typeBytes,
            accountBytes,
            ethAddressBytes,
            tokenIdBytes,
            amountBytes,
            feeBytes,
            nonceBytes
        ]);
        var signature = crypto_1.signTransactionBytes(this.privateKey, msgBytes);
        return {
            type: "Withdraw",
            account: this.address(),
            ethAddress: withdraw.ethAddress,
            token: withdraw.tokenId,
            amount: ethers_1.utils.bigNumberify(withdraw.amount).toString(),
            fee: ethers_1.utils.bigNumberify(withdraw.fee).toString(),
            nonce: withdraw.nonce,
            signature: signature
        };
    };
    Signer.prototype.signSyncCloseAccount = function (close) {
        var type = Buffer.from([4]);
        var account = serializeAddress(this.address());
        var nonce = serializeNonce(close.nonce);
        var msg = Buffer.concat([type, account, nonce]);
        var signature = crypto_1.signTransactionBytes(this.privateKey, msg);
        return {
            type: "Close",
            account: this.address(),
            nonce: close.nonce,
            signature: signature
        };
    };
    Signer.prototype.syncEmergencyWithdrawSignature = function (emergencyWithdraw) {
        var type = Buffer.from([6]);
        var packed_pubkey = crypto_1.serializePointPacked(this.publicKey);
        var account_id = serializeAccountId(emergencyWithdraw.accountId);
        var eth_address = serializeAddress(emergencyWithdraw.ethAddress);
        var token = serializeTokenId(emergencyWithdraw.tokenId);
        var nonce = serializeNonce(emergencyWithdraw.nonce);
        var msg = Buffer.concat([
            type,
            account_id,
            packed_pubkey,
            eth_address,
            token,
            nonce
        ]);
        return Buffer.from(crypto_1.signTransactionBytes(this.privateKey, msg).signature, "hex");
    };
    Signer.fromPrivateKey = function (pk) {
        return new Signer(pk);
    };
    Signer.fromSeed = function (seed) {
        return new Signer(crypto_1.privateKeyFromSeed(seed));
    };
    return Signer;
}());
exports.Signer = Signer;
// Sync or eth address
function serializeAddress(address) {
    var prefixlessAddress = address.startsWith('0x') ? address.substr(2)
        : address.startsWith('sync:') ? address.substr(5)
            : null;
    if (prefixlessAddress === null) {
        throw new Error("ETH address must start with '0x' and Sync address start with 'sync:'");
    }
    var addressBytes = Buffer.from(prefixlessAddress, "hex");
    if (addressBytes.length != 20) {
        throw new Error("Address must be 20 bytes long");
    }
    return addressBytes;
}
function serializeAccountId(accountId) {
    if (accountId < 0) {
        throw new Error("Negative account id");
    }
    if (accountId >= MAX_NUMBER_OF_ACCOUNTS) {
        throw new Error("AccountId is too big");
    }
    var buffer = Buffer.alloc(4);
    buffer.writeUInt32BE(accountId, 0);
    // only 3 bytes
    return buffer.slice(1);
}
function serializeTokenId(tokenId) {
    if (tokenId < 0) {
        throw new Error("Negative tokenId");
    }
    if (tokenId >= MAX_NUMBER_OF_TOKENS) {
        throw new Error("TokenId is too big");
    }
    var buffer = Buffer.alloc(2);
    buffer.writeUInt16BE(tokenId, 0);
    return buffer;
}
function serializeAmountPacked(amount) {
    var bnAmount = new BN(ethers_1.utils.bigNumberify(amount).toString());
    return utils_1.packAmountChecked(bnAmount);
}
function serializeAmountFull(amount) {
    var bnAmount = new BN(ethers_1.utils.bigNumberify(amount).toString());
    return bnAmount.toArrayLike(Buffer, "be", 16);
}
function serializeFeePacked(fee) {
    var bnFee = new BN(ethers_1.utils.bigNumberify(fee).toString());
    return utils_1.packFeeChecked(bnFee);
}
function serializeNonce(nonce) {
    if (nonce < 0) {
        throw new Error("Negative nonce");
    }
    var buff = Buffer.alloc(4);
    buff.writeUInt32BE(nonce, 0);
    return buff;
}
