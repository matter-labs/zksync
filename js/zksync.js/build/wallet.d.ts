import { ContractTransaction, ethers, utils } from "ethers";
import { Provider } from "./provider";
import { Signer } from "./signer";
import { AccountState, Address, TokenLike, Nonce, PriorityOperationReceipt, TransactionReceipt, PubKeyHash } from "./types";
export declare class Wallet {
    signer: Signer;
    ethSigner: ethers.Signer;
    cachedAddress: Address;
    provider: Provider;
    private constructor();
    connect(provider: Provider): this;
    syncTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction>;
    withdrawTo(withdraw: {
        ethAddress?: string;
        token: TokenLike;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction>;
    close(nonce?: Nonce): Promise<Transaction>;
    isCurrentPubkeySet(): Promise<boolean>;
    setCurrentPubkeyWithZksyncTx(nonce?: Nonce, onchainAuth?: boolean): Promise<Transaction>;
    authChangePubkey(nonce?: Nonce): Promise<ContractTransaction>;
    getCurrentPubKeyHash(): Promise<PubKeyHash>;
    getNonce(nonce?: Nonce): Promise<number>;
    address(): Address;
    static fromEthSigner(ethWallet: ethers.Signer, provider: Provider): Promise<Wallet>;
    getAccountState(): Promise<AccountState>;
    getBalance(token: TokenLike, type?: "committed" | "verified"): Promise<utils.BigNumber>;
    getEthereumBalance(token: TokenLike): Promise<utils.BigNumber>;
    depositToSyncFromEthereum(deposit: {
        depositTo: Address;
        token: TokenLike;
        amount: utils.BigNumberish;
        maxFeeInETHToken?: utils.BigNumberish;
    }): Promise<ETHOperation>;
    emergencyWithdraw(withdraw: {
        token: TokenLike;
        maxFeeInETHToken?: utils.BigNumberish;
        accountId?: number;
        nonce?: Nonce;
    }): Promise<ETHOperation>;
}
declare class ETHOperation {
    ethTx: ContractTransaction;
    zkSyncProvider: Provider;
    state: "Sent" | "Mined" | "Committed" | "Verified";
    priorityOpId?: utils.BigNumber;
    constructor(ethTx: ContractTransaction, zkSyncProvider: Provider);
    awaitEthereumTxCommit(): Promise<import("ethers/contract").ContractReceipt>;
    awaitReceipt(): Promise<PriorityOperationReceipt>;
    awaitVerifyReceipt(): Promise<PriorityOperationReceipt>;
}
declare class Transaction {
    txData: any;
    txHash: string;
    sidechainProvider: Provider;
    state: "Sent" | "Committed" | "Verified";
    constructor(txData: any, txHash: string, sidechainProvider: Provider);
    awaitReceipt(): Promise<TransactionReceipt>;
    awaitVerifyReceipt(): Promise<TransactionReceipt>;
}
export {};
