import { utils } from "ethers";

export type SyncAddress = string;

// ETH or ERC20 address
export type Token = "ETH" | string;

export interface SyncAccountState {
    address: SyncAddress;
    id?: number;
    commited: {
        balances: {
            [token: string]: utils.BigNumberish;
        };
        nonce: number;
    };
    verified: {
        balances: {
            [token: string]: utils.BigNumberish;
        };
        nonce: number;
    };
}

export interface Signature {
    pubKey: string;
    signature: string;
}

export interface SyncTransfer {
    from: SyncAddress;
    to: SyncAddress;
    token: number;
    amount: utils.BigNumberish;
    fee: utils.BigNumberish;
    nonce: number;
    signature: Signature;
}

export interface SyncWithdraw {
    account: SyncAddress;
    ethAddress: string;
    token: number;
    amount: utils.BigNumberish;
    fee: utils.BigNumberish;
    nonce: number;
    signature: Signature;
}

export interface SyncCloseAccount {
    account: SyncAddress;
    nonce: number;
    signature: Signature;
}
