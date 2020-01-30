import { ContractTransaction, ethers, utils } from "ethers";
import { ETHProxy, Provider } from "./provider";
import { Signer } from "./signer";
import { AccountState, Address, Token, Nonce, PriorityOperationReceipt, TransactionReceipt } from "./types";
export declare class Wallet {
    signer: Signer;
    provider: Provider;
    ethProxy: ETHProxy;
    constructor(signer: Signer);
    connect(provider: Provider, ethProxy: ETHProxy): this;
    syncTransfer(transfer: {
        to: Address;
        token: Token;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction>;
    withdrawTo(withdraw: {
        ethAddress: string;
        token: Token;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction>;
    close(nonce?: Nonce): Promise<Transaction>;
    getNonce(nonce?: Nonce): Promise<number>;
    address(): Address;
    static fromEthSigner(ethWallet: ethers.Signer, provider?: Provider, ethProxy?: ETHProxy): Promise<Wallet>;
    getAccountState(): Promise<AccountState>;
    getBalance(token: Token, type?: "committed" | "verified"): Promise<utils.BigNumber>;
}
export declare function depositFromETH(deposit: {
    depositFrom: ethers.Signer;
    depositTo: Wallet;
    token: Token;
    amount: utils.BigNumberish;
    maxFeeInETHToken?: utils.BigNumberish;
}): Promise<ETHOperation>;
export declare function emergencyWithdraw(withdraw: {
    withdrawTo: ethers.Signer;
    withdrawFrom: Wallet;
    token: Token;
    maxFeeInETHToken?: utils.BigNumberish;
    accountId?: number;
    nonce?: Nonce;
}): Promise<ETHOperation>;
export declare function getEthereumBalance(ethSigner: ethers.Signer, token: Token): Promise<utils.BigNumber>;
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
