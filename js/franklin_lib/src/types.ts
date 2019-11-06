import { utils } from 'ethers';

export type Address = string;

export interface Token {
    id: number;
    address: string;
    symbol?: string;
}
// token, symbol/eth erc20 contract address, token id
export type TokenLike = Token | string | number;

export type Nonce = number | 'commited';

export interface SidechainAccountBalance {
    address: Address;
    nonce: number;
    balances: any[];
}

export interface SidechainAccountState {
    id?: number;
    commited: SidechainAccountBalance;
    verified: SidechainAccountBalance;
    pending_txs: any[];
}

export interface ETHAccountState {
    onchainBalances: utils.BigNumber[];
    contractBalances: utils.BigNumber[];
}

export interface SidechainInfo {
    contract_address: string;
    tokens: [Token];
}

export interface DepositTx {
    to: Address;
    amount: utils.BigNumberish;
    token: Token;
}

export interface TransferTx {
    from: Address;
    to: Address;
    token: number;
    amount: utils.BigNumberish;
    fee: utils.BigNumberish;
    nonce: number;
}

export interface WithdrawTx {
    account: Address;
    eth_address: string;
    token: number;
    amount: utils.BigNumberish;
    fee: utils.BigNumberish;
    nonce: number;
}

export interface CloseTx {
    account: Address;
    nonce: number;
}

export interface FullExitReq {
    token: number;
    eth_address: string;
    nonce: number;
}
