import 'isomorphic-fetch';
import * as ethers from 'ethers';

type BaseProvider = ethers.ethers.providers.BaseProvider;

export class TimePeriod {
    public timeFrom: Date;
    public timeTo: Date;

    constructor(timeFrom: string, timeTo?: string) {
        this.timeFrom = new Date(timeFrom);
        this.timeTo = timeTo != null ? new Date(timeTo) : new Date();
    }

    public getStartTimeStamp() {
        return Math.floor(this.timeFrom.getTime() / 1000);
    }

    public getEndTimeStamp() {
        return Math.floor(this.timeTo.getTime() / 1000);
    }

    public contains(timeStamp: Date) {
        return +this.timeFrom <= +timeStamp && +timeStamp <= +this.timeTo;
    }

    public less(timeStamp: Date) {
        return timeStamp < this.timeFrom;
    }

    public isValid() {
        return +this.timeFrom <= +this.timeTo;
    }
}

export class TokensCashed {
    private tokenPrice: Map<string, number>;
    private symbolFromID: Map<number, string>;

    constructor() {
        this.tokenPrice = new Map();
        this.symbolFromID = new Map();
    }

    public addToken(tokenSymbol: string, tokenID: number, tokenPrice: number) {
        this.tokenPrice.set(tokenSymbol, tokenPrice);
        this.symbolFromID.set(tokenID, tokenSymbol);
    }

    public getTokenSymbol(id: number) {
        return this.symbolFromID.get(id);
    }

    public getTokenPrice(tokenSymbol: string) {
        return this.tokenPrice.get(tokenSymbol);
    }
}

export async function chainTransactionFee(ethProvider: BaseProvider, txHash: string) {
    const transaction = await ethProvider.getTransaction(txHash);
    const transactionRequest = await ethProvider.getTransactionReceipt(txHash);

    if (transaction == null || transactionRequest == null) return 0;

    const feeWei = transactionRequest.gasUsed.mul(transaction.gasPrice);
    const transactionFee = Number(ethers.utils.formatEther(feeWei));

    return transactionFee;
}

export function getTransactionFee(transaction: any) {
    if (transaction == null || transaction.op == null || transaction.op.fee == null) return ethers.BigNumber.from(0);

    return ethers.BigNumber.from(transaction.op.fee);
}

export function getTransactionTokenID(transaction: any) {
    if (transaction == null || transaction.op == null) return 0;

    if (transaction.op.token != null) return Number(transaction.op.token);

    if (transaction.op.priority_op != null && transaction.op.priority_op.token != null)
        return Number(transaction.op.priority_op.token);

    return 0;
}

export function correctTransactionWithFee(transaction: any) {
    return transaction != null && transaction.op != null && transaction.op.fee != null;
}

export async function getBlockInterval(etherscanApiURL: string, etherscanApiKey: string, timePeriod: TimePeriod) {
    const startBlockUrl =
        `${etherscanApiURL}/api` +
        `?module=block` +
        `&action=getblocknobytime` +
        `&timestamp=${timePeriod.getStartTimeStamp()}` +
        `&closest=after` +
        `&apikey=${etherscanApiKey}`;

    const endBlockUrl =
        `${etherscanApiURL}/api` +
        `?module=block` +
        `&action=getblocknobytime` +
        `&timestamp=${timePeriod.getEndTimeStamp()}` +
        `&closest=before` +
        `&apikey=${etherscanApiKey}`;

    const responseStartBlock = await fetch(startBlockUrl);
    const responseEndBlock = await fetch(endBlockUrl);

    const startBlock = (await responseStartBlock.json()).result;
    const endBlock = (await responseEndBlock.json()).result;

    if (startBlock == null || endBlock == null) throw new Error(`Failed to get block by time from ${etherscanApiURL}`);

    return { startBlock, endBlock };
}
