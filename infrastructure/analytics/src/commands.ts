import 'isomorphic-fetch';
import { Network, TokensInfo } from './types';
import * as zksync from 'zksync';
import * as ethers from 'ethers';
import * as utils from './utils';

export async function currentBalances(network: Network, operator_address: string) {
    const zksProvider = await zksync.getDefaultProvider(network, 'HTTP');
    const ethProvider =
        network == 'localhost' ? new ethers.providers.JsonRpcProvider() : ethers.getDefaultProvider(network);

    const balances: TokensInfo = { total: { eth: 0, usd: 0 } };

    const eth_price = await zksProvider.getTokenPrice('ETH');
    const tokens = await zksProvider.getTokens();

    for (const token in tokens) {
        if (zksProvider.tokenSet.resolveTokenSymbol(token) === 'MLTT' || zksync.utils.isTokenETH(token)) continue;

        const tokenAddress = tokens[token].address;

        const erc20contract = new ethers.Contract(
            tokenAddress,
            zksync.utils.IERC20_INTERFACE as ethers.ethers.ContractInterface,
            ethProvider
        );

        const tokenPrice = await zksProvider.getTokenPrice(token);
        const contractBalance = await erc20contract.balanceOf(operator_address);
        const tokenAmount = Number(zksProvider.tokenSet.formatToken(token, contractBalance));

        const usd_cost = tokenPrice * tokenAmount;
        const eth_cost = usd_cost / eth_price;

        balances.total.eth += eth_cost;
        balances.total.usd += usd_cost;

        balances[token] = {
            amount: tokenAmount,
            eth: eth_cost,
            usd: usd_cost
        };
    }
    return balances;
}

export async function collectedFees(network: Network, providerAddress: string, timePeriod: utils.TimePeriod) {
    const MAX_LIMIT = 100; // maximum number of blocks that the server returns in one request
    let currentBlock = 999_999_999; // the maximum block number that we request from the server
    let currentBlockTime = new Date();

    if (!timePeriod.isValid()) throw new Error(`Error time period ${timePeriod.timeFrom} - ${timePeriod.timeTo}`);

    const zksProvider = await zksync.getDefaultProvider(network);
    const ethProvider =
        network == 'localhost' ? new ethers.providers.JsonRpcProvider() : ethers.getDefaultProvider(network);

    const eth_price = await zksProvider.getTokenPrice('ETH');
    const tokens = await zksProvider.getTokens();

    const senderAccountStat = { eth: 0, usd: 0 };
    const tokensStat: TokensInfo = { total: { eth: 0, usd: 0 } };

    // structure that stores data about each token from zSync
    // so as not to request the server many times for the same data
    const tokensCashed = new utils.TokensCashed();

    for (const token in tokens) {
        const tokenSymbol = zksProvider.tokenSet.resolveTokenSymbol(token);
        const todenId = zksProvider.tokenSet.resolveTokenId(token);
        const tokenPrice = await zksProvider.getTokenPrice(token);

        tokensCashed.addToken(tokenSymbol, todenId, tokenPrice);
        tokensStat[token] = { amount: 0, eth: 0, usd: 0 };
    }

    // traverse all blocks starting from the last one
    while (!timePeriod.less(currentBlockTime)) {
        const blockUrl = `${providerAddress}/api/v0.1/blocks?limit=${MAX_LIMIT}&max_block=${currentBlock}`;
        const response = await fetch(blockUrl);
        const blocks = await response.json();

        if (blocks == null) break;

        for (const block of blocks) {
            console.log(
                `Block number: ${block.block_number}, commit Txhash: ${block.commit_tx_hash}, verify Txhash: ${block.verify_tx_hash}`
            );
            // skip uncommited blocks
            if (block.committed_at == null) continue;

            currentBlock = block.block_number;
            currentBlockTime = new Date(block.committed_at);

            if (timePeriod.less(currentBlockTime)) break;

            const commitTransactionFee = await utils.chainTransactionFee(ethProvider, block.commit_tx_hash);

            // update statistics for `commit` operation in L1
            senderAccountStat.eth += commitTransactionFee;
            senderAccountStat.usd += commitTransactionFee * eth_price;

            // skip unverified blocks
            if (block.verified_at == null) continue;

            currentBlockTime = new Date(block.verified_at);
            if (timePeriod.less(currentBlockTime)) break;

            const verifyTransactionFee = await utils.chainTransactionFee(ethProvider, block.verify_tx_hash);

            // update statistics for `verify` operation in L1
            senderAccountStat.eth += verifyTransactionFee;
            senderAccountStat.usd += verifyTransactionFee * eth_price;

            // Each block includes many transactions
            // Some transactions include a fee that operator collect
            const transactionUrl = `${providerAddress}/api/v0.1/blocks/${currentBlock}/transactions`;
            const response = await fetch(transactionUrl);
            const transactions = await response.json();

            if (transactions == null) continue;

            for (const transaction of transactions) {
                const transactionTime = new Date(transaction.created_at);

                // TODO: handle fee for `CompleteWithdrawals` operation in L1
                // wait for update API

                // some transactions that are included in the block do not contain fee
                if (utils.correctTransactionWithFee(transaction) && timePeriod.contains(transactionTime)) {
                    const transactionFee = utils.getTransactionFee(transaction);

                    const tokenID = utils.getTransactionTokenID(transaction);
                    const tokenSymbol = tokensCashed.getTokenSymbol(tokenID);
                    const tokenPrice = tokensCashed.getTokenPrice(tokenSymbol);
                    const tokenAmount = Number(zksProvider.tokenSet.formatToken(tokenSymbol, transactionFee));

                    //update statistics on collected tokens
                    tokensStat[tokenSymbol].amount += tokenAmount;
                    tokensStat[tokenSymbol].usd += tokenAmount * tokenPrice;
                    tokensStat[tokenSymbol].eth += (tokenAmount * tokenPrice) / eth_price;

                    tokensStat.total.usd += tokenAmount * tokenPrice;
                    tokensStat.total.eth += (tokenAmount * tokenPrice) / eth_price;
                }
            }
        }
    }

    return Object.assign({ 'spent by SENDER ACCOUNT': senderAccountStat }, { 'collected fees': tokensStat });
}

export async function collectedTokenLiquidations(
    network: Network,
    operatorAddress: string,
    timePeriod: utils.TimePeriod,
    etherscan_api_key: string
) {
    if (!timePeriod.isValid()) throw new Error(`Error time period ${timePeriod.timeFrom} - ${timePeriod.timeTo}`);

    // To view all transactions outgoing from the account use the Etherscan provider
    const ethProvider = new ethers.providers.EtherscanProvider(network, etherscan_api_key);

    let liquidationAmount = 0;
    let history: ethers.ethers.providers.TransactionResponse[];

    // Etherscan API has limits on the number of transactions in one request
    // so request transactions until getting an empty list
    do {
        const { startBlock, endBlock } = await utils.getBlockInterval(
            ethProvider.baseUrl,
            etherscan_api_key,
            timePeriod
        );

        history = await ethProvider.getHistory(operatorAddress, startBlock, endBlock);

        for (const transaction of history) {
            console.log(`Tx hash: ${transaction.hash}`);

            // save the current time as the last viewed transaction + 1 second
            timePeriod.timeFrom = new Date(transaction.timestamp * 1000 + 1000);

            if (transaction.from == null || transaction.from.toLocaleLowerCase() != operatorAddress) continue;

            const transactionValueWei = transaction.value;
            const transactionValue = Number(ethers.utils.formatEther(transactionValueWei));

            liquidationAmount += transactionValue;
        }
    } while (history.length > 0 && timePeriod.isValid());

    return { 'Total amount of ETH': liquidationAmount };
}
