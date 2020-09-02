export type Network = "localhost" | "mainnet" | "ropsten" | "rinkeby";

export const ALL_NETWORKS: Network[] = [
    'localhost',
    'mainnet',
    'ropsten',
    'rinkeby',
];

export interface Wallet {
    privkey: string
    address: string,
}

export interface Config {
    network: Network,
    defaultWallet: string | null,
    wallets: Wallet[]
}

