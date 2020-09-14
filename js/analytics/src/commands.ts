import * as zksync from 'zksync';
import * as types from './types';
import * as ethers from 'ethers';

export async function currentBalances(network: types.Network, operator_address: string, web3_url?: string) {
    const zksProvider = await zksync.getDefaultProvider(network, "HTTP");
    const ethProvider = web3_url? ethers.getDefaultProvider() : new ethers.providers.JsonRpcProvider(web3_url);

    let balances: types.Tokens = {total: {eth: 0, usd: 0}};

    const eth_price = await zksProvider.getTokenPrice("ETH");
    const tokens = await zksProvider.getTokens();

    for (const token in tokens) {
        if (zksProvider.tokenSet.resolveTokenSymbol(token) === "MLTT" || zksync.utils.isTokenETH(token))
           continue;

        const tokenAddress = tokens[token].address;
        const erc20contract = new ethers.Contract(tokenAddress, zksync.utils.IERC20_INTERFACE, ethProvider);
        
        const tokenPrice = await zksProvider.getTokenPrice(token); 
        const tokenAmount = await erc20contract.balanceOf(operator_address);
        const numberOfTokens = Number(zksProvider.tokenSet.formatToken(token, tokenAmount));

        const usd_cost = tokenPrice * numberOfTokens;
        const eth_cost = usd_cost / eth_price;

        balances.total.eth += eth_cost;
        balances.total.usd += usd_cost;

        balances[token] = {
            amount: numberOfTokens,
            eth: eth_cost,
            usd: usd_cost
        };
    }
    return balances;
}

export async function collectedFees(network: types.Network, providerAddress: string, timePeriod: types.TimePeriod, web3_url?: string) {
    const MAX_LIMIT = 100; // maximum number of blocks that the server returns in one request
    let currentBlock = 999_999_999;
    let currentBlockTime = new Date();

    const zksProvider = await zksync.getDefaultProvider(network, "HTTP");
    const ethProvider = web3_url? ethers.getDefaultProvider() : new ethers.providers.JsonRpcProvider(web3_url);
    
    const eth_price = await zksProvider.getTokenPrice("ETH");

    let senderAccountStat = {"spent by SENDER ACCOUNT": {"eth": 0, "usd": 0}}; 
    let tokensStat: types.Tokens = {total: {eth: 0, usd: 0}};

    let resolveSymbolFromID, tokenPrice = {}; 
    for (const token in await zksProvider.getTokens()) {
        resolveSymbolFromID[zksProvider.tokenSet.resolveTokenId(token)] = token;
        tokenPrice[token] = await zksProvider.getTokenPrice(token);  
        tokensStat[token] = {amount: 0, eth: 0, usd: 0};
    }
    
    while(!timePeriod.less(currentBlockTime))
    {
        const blockUrl = `${providerAddress}/api/v0.1/blocks?limit=${MAX_LIMIT}&max_block=${currentBlock}`;
        const response = await fetch(blockUrl);
        const blocks = await response.json();
        
        if (blocks == null)
            break;

        for(const block of blocks) {
            currentBlock = block.block_number;
            currentBlockTime = new Date(block.committed_at);

            if(timePeriod.less(currentBlockTime))
                break;

            const commitTransaction = await ethProvider.getTransaction(block.commit_tx_hash);
            const commitTransactionRequest = await ethProvider.getTransactionReceipt(block.commit_tx_hash);
            
            if(commitTransaction == null || commitTransactionRequest == null)
                continue;

            const commitFeeWei = commitTransactionRequest.gasUsed.mul(commitTransaction.gasPrice);
            const commitTransactionFee = Number(ethers.utils.formatEther(commitFeeWei));
            
            senderAccountStat["spent by SENDER ACCOUNT"].eth += commitTransactionFee;
            senderAccountStat["spent by SENDER ACCOUNT"].usd += commitTransactionFee * eth_price;

            if(block.verified_at == null) // skip unverified blocks
                continue;

            currentBlockTime = new Date(block.verified_at);

            if(timePeriod.less(currentBlockTime))
                break;

            const verifyTransaction = await ethProvider.getTransaction(block.verify_tx_hash);
            const verifyTransactionRequest = await ethProvider.getTransactionReceipt(block.verify_tx_hash);
           
            if(verifyTransaction == null || verifyTransactionRequest == null)
                continue;

            const verifyFeeWei = commitTransactionRequest.gasUsed.mul(commitTransaction.gasPrice);
            const verifyTransactionFee = Number(ethers.utils.formatEther(verifyFeeWei));

            senderAccountStat["spent by SENDER ACCOUNT"].eth += verifyTransactionFee;
            senderAccountStat["spent by SENDER ACCOUNT"].usd += verifyTransactionFee * eth_price;

            const transactionUrl = `${providerAddress}/api/v0.1/blocks/${currentBlock}/transactions`;
            const response = await fetch(transactionUrl);
            const transactions = await response.json();
                
            if(transactions == null)
                continue;
            
            for(const transaction of transactions)
            {
                if(transaction == null || transaction.op == null || transaction.op.fee == null || transaction.op.token == null)
                    continue;

                const transactionTime = new Date(transaction.created_at);
                if(timePeriod.inTime(transactionTime)) 
                {
                    const tokenSymbol = resolveSymbolFromID[transaction.op.token];
                    const numberOfTokens = Number(zksProvider.tokenSet.formatToken(tokenSymbol, ethers.BigNumber.from(transaction.op.fee)));

                    tokensStat[tokenSymbol].amount += numberOfTokens;    
                    tokensStat[tokenSymbol].usd += numberOfTokens * tokenPrice[tokenSymbol];    
                }
            }
        }
    }
    
    return Object.assign(senderAccountStat, {"collected fees" : tokensStat});
}

export async function collectedTokenLiquidations(network: types.Network, operatorAddress: string, time: types.TimePeriod) {
    let provider = new ethers.providers.EtherscanProvider();
    let history = await provider.getHistory(operatorAddress);
    let liquidationInfo = {"Total amount of ETH": 0}; 

    for(const transaction in history.reverse())
    {
    if(history[transaction].timestamp == null)
        continue;
    
    const transactionTime = new Date(history[transaction].timestamp * 1000);

    if(time.less(transactionTime))
        break;
    
    const transactionValueWei = history[transaction].value;
    const transactionValue = Number(ethers.utils.formatEther(transactionValueWei));
    liquidationInfo["Total amount of ETH"] += transactionValue;
    }
    return liquidationInfo;
}
