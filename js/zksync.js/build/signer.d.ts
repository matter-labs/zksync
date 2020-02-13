/// <reference types="node" />
import { curve } from "elliptic";
import { utils } from "ethers";
import BN = require("bn.js");
import { Address, CloseAccount, PubKeyHash, Transfer, Withdraw } from "./types";
export declare class Signer {
    readonly privateKey: BN;
    readonly publicKey: curve.edwards.EdwardsPoint;
    private constructor();
    pubKeyHash(): PubKeyHash;
    signSyncTransfer(transfer: {
        from: Address;
        to: Address;
        tokenId: number;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce: number;
    }): Transfer;
    signSyncWithdraw(withdraw: {
        from: Address;
        ethAddress: string;
        tokenId: number;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce: number;
    }): Withdraw;
    signSyncCloseAccount(close: {
        nonce: number;
    }): CloseAccount;
    static fromPrivateKey(pk: BN): Signer;
    static fromSeed(seed: Buffer): Signer;
}
export declare function serializeAddress(address: Address | string): Buffer;
export declare function serializeNonce(nonce: number): Buffer;
