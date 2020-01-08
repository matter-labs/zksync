/// <reference types="node" />
import { curve } from "elliptic";
import { utils } from "ethers";
import BN = require("bn.js");
import { Address, CloseAccount, Transfer, Withdraw } from "./types";
export declare class Signer {
    readonly privateKey: BN;
    readonly publicKey: curve.edwards.EdwardsPoint;
    private constructor();
    address(): Address;
    signSyncTransfer(transfer: {
        to: Address;
        tokenId: number;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce: number;
    }): Transfer;
    signSyncWithdraw(withdraw: {
        ethAddress: string;
        tokenId: number;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce: number;
    }): Withdraw;
    signSyncCloseAccount(close: {
        nonce: number;
    }): CloseAccount;
    syncEmergencyWithdrawSignature(emergencyWithdraw: {
        accountId: number;
        ethAddress: string;
        tokenId: number;
        nonce: number;
    }): Buffer;
    static fromPrivateKey(pk: BN): Signer;
    static fromSeed(seed: Buffer): Signer;
}
