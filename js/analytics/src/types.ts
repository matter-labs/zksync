
export type Network = "localhost" | "rinkeby" | "ropsten" | "mainnet";

export interface config {
    network : Network,
    rest_api_address: string
    zksync_contract_address: string,
    operator_commit_address: string,
    operator_fee_address: string,
}

export class TimePeriod {
    public timeFrom: number;
    public timeTo: number;

    constructor(timeFrom: string, timeTo?: string) {
        this.timeFrom = Date.parse(timeFrom);
        this.timeTo = timeTo? Date.parse(timeTo): new Date().valueOf();
    };

    inTime(timeStamp: number){
        return timeStamp >= this.timeFrom && timeStamp <= this.timeTo;
    }    
}