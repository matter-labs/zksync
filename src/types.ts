import { utils } from "ethers";

export type Address = string;

// ETH or ERC20 address
export type Token = "ETH" | string;

export interface AccountState {
    address: Address;
    id?: number;
    committed: {
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

export interface Transfer {
    from: Address;
    to: Address;
    token: number;
    amount: utils.BigNumberish;
    fee: utils.BigNumberish;
    nonce: number;
    signature: Signature;
}

export interface Withdraw {
    account: Address;
    ethAddress: string;
    token: number;
    amount: utils.BigNumberish;
    fee: utils.BigNumberish;
    nonce: number;
    signature: Signature;
}

export interface CloseAccount {
    account: Address;
    nonce: number;
    signature: Signature;
}

export interface BlockInfo {
    blockNumber: number;
    committed: boolean;
    verified: boolean;
}

export interface TransactionReceipt {
    executed: boolean;
    success?: boolean;
    failReason?: string;
    block?: BlockInfo;
}

export interface PriorityOperationReceipt {
    executed: boolean;
    block?: BlockInfo;
}
