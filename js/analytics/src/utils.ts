import {Provider} from 'zksync';

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
        return timeStamp < this.timeFrom;
    }
}

export class TokensCashed {
    tokenPrice: Map<string, number>;
    symbolFromID: Map<number, string>;

    addToken(tokenSymbol: string, tokenID: number, tokenPrice: number){
        this.tokenPrice[tokenSymbol] = tokenPrice;
        this.symbolFromID[tokenID] = tokenSymbol;
    }

    getTokenSymbol(id: number)
    {
        return this.symbolFromID[id];
    }

    getTokenPrice(tokenSymbol: string)
    {
        return this.tokenPrice[tokenSymbol];
    }
}

export async function getBlockInterval(timePeriod: TimePeriod) {
    const startBlockUrl = "https://api.etherscan.io/api" +
        `?module=block` +
        `&action=getblocknobytime` +
        `&timestamp=${timePeriod.timeFrom.valueOf()}` +
        `&closest=after`;

    const endBlockUrl = "https://api.etherscan.io/api" +
        `?module=block` +
        `&action=getblocknobytime` +
        `&timestamp=${timePeriod.timeTo.valueOf()}` +
        `&closest=before`;
    

    const responseStartBlock = await fetch(startBlockUrl);
    const responseEndBlock = await fetch(endBlockUrl);

    const startBlock = await responseStartBlock.json();
    const endBlock = await responseEndBlock.json();

    return {startBlock, endBlock};
}