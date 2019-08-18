/// <reference types="node" />
import BN = require('bn.js');
import { curve } from 'elliptic';
import EdwardsPoint = curve.edwards.EdwardsPoint;
import { ethers } from 'ethers';
declare type BigNumber = ethers.utils.BigNumber;
declare type BigNumberish = ethers.utils.BigNumberish;
export declare type Address = string;
declare class FranklinProvider {
    providerAddress: string;
    constructor(providerAddress?: string);
    submitTx(tx: any): Promise<any>;
    getTokens(): Promise<any>;
    getState(address: Address): Promise<FranklinAccountState>;
}
export interface Token {
    id: number;
    address: string;
    symbol?: string;
}
export interface FranklinAccountState {
    address: Address;
    nonce: number;
    balances: BigNumber[];
}
export interface FranklinAccountState {
    id?: number;
    commited: FranklinAccountState;
    verified: FranklinAccountState;
    pending_txs: any[];
}
interface ETHAccountState {
    onchainBalances: BigNumber[];
    contractBalances: BigNumber[];
    lockedBlocksLeft: number[];
}
export declare class Wallet {
    provider: FranklinProvider;
    ethWallet: ethers.Signer;
    ethAddress: string;
    address: Address;
    privateKey: BN;
    publicKey: EdwardsPoint;
    supportedTokens: Token[];
    franklinState: FranklinAccountState;
    ethState: ETHAccountState;
    constructor(seed: Buffer, provider: FranklinProvider, ethWallet: ethers.Signer, ethAddress: string);
    depositOnchain(token: Token, amount: BigNumberish): Promise<any>;
    depositOffchain(token: Token, amount: BigNumberish, fee: BigNumberish): Promise<any>;
    widthdrawOnchain(token: Token, amount: BigNumberish): Promise<any>;
    widthdrawOffchain(token: Token, amount: BigNumberish, fee: BigNumberish): Promise<any>;
    transfer(address: Address, token: Token, amount: BigNumberish, fee: BigNumberish): Promise<any>;
    getNonce(): Promise<number>;
    static fromEthWallet(wallet: ethers.Signer): Promise<Wallet>;
    fetchEthState(): Promise<void>;
    fetchFranklinState(): Promise<void>;
    updateState(): Promise<void>;
    waitPendingTxsExecuted(): Promise<void>;
}
export {};
