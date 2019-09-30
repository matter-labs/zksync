/// <reference types="node" />
import BN = require('bn.js');
import { curve } from 'elliptic';
import { ethers } from 'ethers';
import edwards = curve.edwards;
declare type BigNumber = ethers.utils.BigNumber;
declare type BigNumberish = ethers.utils.BigNumberish;
export declare type Address = Buffer;
export declare type AddressLike = Buffer | string;
export declare function toAddress(addressLike: AddressLike): Address;
export declare class FranklinProvider {
    providerAddress: string;
    contractAddress: string;
    constructor(providerAddress?: string, contractAddress?: string);
    static prepareTransferRequestForNode(tx: TransferTx, signature: any): any;
    static prepareWithdrawRequestForNode(tx: WithdrawTx, signature: any): any;
    static prepareCloseRequestForNode(tx: CloseTx, signature: any): any;
    submitTx(tx: any): Promise<any>;
    getTokens(): Promise<any>;
    getTransactionsHistory(address: Address): Promise<any>;
    getState(address: Address): Promise<FranklinAccountState>;
    getTxReceipt(tx_hash: any): Promise<any>;
}
export interface Token {
    id: number;
    address: string;
    symbol?: string;
}
export interface FranklinAccountBalanceState {
    address: Address;
    nonce: number;
    balances: BigNumber[];
}
export interface FranklinAccountState {
    id?: number;
    commited: FranklinAccountBalanceState;
    verified: FranklinAccountBalanceState;
    pending_txs: any[];
}
interface ETHAccountState {
    onchainBalances: BigNumber[];
    contractBalances: BigNumber[];
}
export interface TransferTx {
    from: Address;
    to: Address;
    token: number;
    amount: BigNumberish;
    fee: BigNumberish;
    nonce: number;
}
export interface WithdrawTx {
    account: Address;
    eth_address: String;
    token: number;
    amount: BigNumberish;
    fee: BigNumberish;
    nonce: number;
}
export interface CloseTx {
    account: Address;
    nonce: number;
}
export interface FullExitReq {
    token: number;
    eth_address: String;
    nonce: number;
}
export declare class WalletKeys {
    privateKey: BN;
    publicKey: edwards.EdwardsPoint;
    constructor(privateKey: BN);
    signTransfer(tx: TransferTx): {
        pub_key: string;
        sign: string;
    };
    signWithdraw(tx: WithdrawTx): {
        pub_key: string;
        sign: string;
    };
    signClose(tx: CloseTx): {
        pub_key: string;
        sign: string;
    };
    signFullExit(op: FullExitReq): Buffer;
}
export declare class Wallet {
    provider: FranklinProvider;
    ethWallet: ethers.Signer;
    ethAddress: string;
    address: Address;
    walletKeys: WalletKeys;
    supportedTokens: Token[];
    franklinState: FranklinAccountState;
    ethState: ETHAccountState;
    pendingNonce: number;
    constructor(seed: Buffer, provider: FranklinProvider, ethWallet: ethers.Signer, ethAddress: string);
    deposit(token: Token, amount: BigNumberish): Promise<any>;
    waitTxReceipt(tx_hash: any): Promise<any>;
    widthdrawOnchain(token: Token, amount: BigNumberish): Promise<any>;
    widthdrawOffchain(token: Token, amount: BigNumberish, fee: BigNumberish): Promise<any>;
    emergencyWithdraw(token: Token): Promise<any>;
    transfer(to: AddressLike, token: Token, amount: BigNumberish, fee: BigNumberish): Promise<any>;
    close(): Promise<any>;
    getNonce(): Promise<number>;
    static fromEthWallet(wallet: ethers.Signer, franklinProvider?: FranklinProvider): Promise<Wallet>;
    fetchEthState(): Promise<void>;
    fetchFranklinState(): Promise<void>;
    updateState(): Promise<void>;
    waitPendingTxsExecuted(): Promise<void>;
}
export {};
