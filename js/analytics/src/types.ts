
export type Network = "localhost" | "rinkeby" | "ropsten" | "mainnet";

export interface config {
    network : Network,
    rest_api_address: string
    zksync_contract_address: string,
    operator_commit_address: string,
    operator_fee_address: string,
    web3_url?: string 
}

export class TimePeriod {
    public timeFrom: Date;
    public timeTo: Date;

    constructor(timeFrom: string, timeTo?: string) {
        this.timeFrom = new Date(timeFrom);
        this.timeTo = timeTo? new Date(timeTo): new Date();
    };

    inTime(timeStamp: Date) {
        return +this.timeFrom <= +timeStamp && +this.timeTo >= +timeStamp;
    }  

    less(timeStamp: Date) {
        return this.timeFrom > timeStamp;
    }
}

export interface Tokens {
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

export interface Block {
    block_number: number;
    new_state_root: string;
    block_size: number;
    commit_tx_hash: string;
    verify_tx_hash?: string;
    committed_at: string;
    verified_at?: string;
};