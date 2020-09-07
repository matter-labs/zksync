export type Network = 'localhost' | 'mainnet' | 'ropsten' | 'rinkeby';

export const ALL_NETWORKS: Network[] = ['localhost', 'mainnet', 'ropsten', 'rinkeby'];

export interface Wallet {
    privkey: string;
    address: string;
}

export interface Config {
    network: Network;
    defaultWallet: string | null;
    wallets: Wallet[];
}

export interface AccountInfo {
    address: string;
    network: Network;
    account_id?: number;
    nonce: number;
    balances: {
        [token: string]: string;
    };
}

export interface TxInfo {
    network: Network;
    transaction: null | {
        status: 'error' | 'success';
        from: string;
        to: string;
        hash: string;
        operation: string;
        token: string;
        amount: string;
        fee: string;
        nonce: number;
    };
}

export interface TransferInfo {
    from: string;
    to: string;
    token: string;
    amount: string;
}
