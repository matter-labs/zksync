export type Network = "localhost" | "rinkeby" | "ropsten" | "mainnet";

export interface Config {
    network: Network;
    rest_api_address: string;
    operator_commit_address: string;
    operator_fee_address: string;
    etherscan_api_address?: string;
}

export interface TokensInfo {
    total: {
        eth: number;
        usd: number;
    };
    [token: string]: {
        amount?: number;
        eth: number;
        usd: number;
    };
}
