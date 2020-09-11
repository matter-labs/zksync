import * as zksync from 'zksync';
import * as types from './types';
import * as ethers from 'ethers';

export async function currentBalances(network: types.Network, operator_address: string) {
    const provider = await zksync.getDefaultProvider(network);
    const state = await provider.getState(operator_address);
    
    let balances: {[token: string] : {eth: number, usd: number}} = {"total": {"eth": 0, "usd": 0}};
    const eth_price = await provider.getTokenPrice("ETH");

    for (const token in state.verified.balances) {
        const tokenPrice = await provider.getTokenPrice(token); 
        const tokenAmount = state.verified.balances[token].toString();

        const numberOfTokens = Number(provider.tokenSet.formatToken(token, tokenAmount));
        const usd_cost = tokenPrice * numberOfTokens;
        const eth_cost = usd_cost / eth_price;

        balances["total"]["eth"] += eth_cost;
        balances["total"]["usd"] += usd_cost;

        balances[token] = {
            "eth": eth_cost,
            "usd": usd_cost
        };
    }
    await provider.disconnect();

    return balances;
}

export async function collectedFees(network: types.Network, providerAddress: string, time: types.TimePeriod) {
    const provider = await zksync.getDefaultProvider(network);
    const ethProvider = ethers.getDefaultProvider(network);
    
    const eth_price = await provider.getTokenPrice("ETH");
    
    const MAX_LIMIT = 100; // maximum number of blocks that the server returns in one request
    let currentBlock = 999_999_999;
    let currentBlockTimeStamp = new Date().valueOf();

    let statistics: {[token: string] : {eth: number, usd: number}} = {"SENDER_ACCOUNT": {"eth": 0, "usd": 0}};
    let tokensStat = {};
    let resolveSymbolFromID = {};
    for (const token in await provider.getTokens()) {
        resolveSymbolFromID[provider.tokenSet.resolveTokenId(token)] = token;
        tokensStat[token] = {"token": 0, "usd": 0};
    }
        
    while(currentBlockTimeStamp > time.timeFrom)
    {
        const blockUrl = `${providerAddress}/api/v0.1/blocks?limit=${MAX_LIMIT}&max_block=${currentBlock}`;
        const response = await fetch(blockUrl);
        const blocks = await response.json();
        
        if (blocks === null)
            break;
        
        for(const block of blocks) {
            // skip unverified blocks
            if(block.verified_at == null)
                continue;
            
            currentBlock = block.block_number;
            currentBlockTimeStamp =  Date.parse(block.verified_at);

            if(currentBlockTimeStamp < time.timeFrom)
                break;

            const commitTransaction = await ethProvider.getTransaction(block.commit_tx_hash);
            const verifyTransaction = await ethProvider.getTransaction(block.verify_tx_hash);
            const verifyTransactionRequest = await ethProvider.getTransactionReceipt(block.verify_tx_hash);
            const commitTransactionRequest = await ethProvider.getTransactionReceipt(block.commit_tx_hash);

            if(commitTransaction == null || verifyTransactionRequest == null || verifyTransaction == null || commitTransactionRequest == null)
                continue;

            const commitTransactionFee = Number(ethers.utils.formatEther(commitTransactionRequest.gasUsed.mul(commitTransaction.gasPrice)));
            const verifyTransactionFee = Number(ethers.utils.formatEther(verifyTransactionRequest.gasUsed.mul(commitTransaction.gasPrice)));

            statistics["SENDER_ACCOUNT"]["eth"] += verifyTransactionFee + commitTransactionFee;
            statistics["SENDER_ACCOUNT"]["usd"] += (verifyTransactionFee + commitTransactionFee) * eth_price;
            
            const transactionUrl = `${providerAddress}/api/v0.1/blocks/${currentBlock}/transactions`;
            const response = await fetch(transactionUrl);
            const transactions = await response.json();
                
            if(transactions == null)
                continue;
            
            for(const transaction of transactions)
            {
                if(transaction == null || transaction.op == null || transaction.op.fee == null || transaction.op.token == null)
                    continue;

                const transactionTimeStamp = new Date(transaction.created_at).valueOf();
                if(time.inTime(transactionTimeStamp)) 
                {
                    const tokenSymbol = resolveSymbolFromID[transaction.op.token];
                    const tokenPrice = await provider.getTokenPrice(tokenSymbol);        

                    tokensStat[tokenSymbol]["token"] += Number(transaction.op.fee);    
                    tokensStat[tokenSymbol]["usd"] += Number(transaction.op.fee) * tokenPrice;    
                }
            }
        }
    }
    const result = {statistics, tokensStat};
    return result;
}

export async function collectedTokenLiquidations() {
    // TODO
}
