import "isomorphic-fetch";
import { Network, TokensInfo } from "./types";
import * as zksync from "zksync";
import * as ethers from "ethers";
import * as utils from "./utils";

export async function currentBalances(network: Network, operator_address: string, web3_url?: string) {
    const zksProvider = await zksync.getDefaultProvider(network, "HTTP");
    const ethProvider = web3_url
        ? new ethers.providers.JsonRpcProvider(web3_url, network)
        : ethers.getDefaultProvider(network);

    let balances: TokensInfo = { total: { eth: 0, usd: 0 } };

    const eth_price = await zksProvider.getTokenPrice("ETH");
    const tokens = await zksProvider.getTokens();

    for (const token in tokens) {
        if (zksProvider.tokenSet.resolveTokenSymbol(token) === "MLTT" || zksync.utils.isTokenETH(token)) continue;

        const tokenAddress = tokens[token].address;
        const erc20contract = new ethers.Contract(tokenAddress, zksync.utils.IERC20_INTERFACE, ethProvider);

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
            usd: usd_cost,
        };
    }
    return balances;
}

export async function collectedFees(
    network: Network,
    providerAddress: string,
    timePeriod: utils.TimePeriod,
    web3_url?: string
) {
    const MAX_LIMIT = 100; // maximum number of blocks that the server returns in one request
    let currentBlock = 999_999_999; // the maximum block number that we request from the server
    let currentBlockTime = new Date();

    const zksProvider = await zksync.getDefaultProvider(network);
    const ethProvider = web3_url
        ? new ethers.providers.JsonRpcProvider(web3_url, network)
        : ethers.getDefaultProvider(network);

    const eth_price = await zksProvider.getTokenPrice("ETH");
    const tokens = await zksProvider.getTokens();

    let senderAccountStat = { "spent by SENDER ACCOUNT": { eth: 0, usd: 0 } };
    let tokensStat: TokensInfo = { total: { eth: 0, usd: 0 } };
    let tokensCashed = new utils.TokensCashed();

    for (const token in tokens) {
        const tokenSymbol = zksProvider.tokenSet.resolveTokenSymbol(token);
        const todenId = zksProvider.tokenSet.resolveTokenId(token);
        const tokenPrice = await zksProvider.getTokenPrice(token);

        tokensCashed.addToken(tokenSymbol, todenId, tokenPrice);
        tokensStat[token] = { amount: 0, eth: 0, usd: 0 };
    }

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

            senderAccountStat["spent by SENDER ACCOUNT"].eth += commitTransactionFee;
            senderAccountStat["spent by SENDER ACCOUNT"].usd += commitTransactionFee * eth_price;

            // skip unverified blocks
            if (block.verified_at == null) continue;

            currentBlockTime = new Date(block.verified_at);
            if (timePeriod.less(currentBlockTime)) break;

            const verifyTransactionFee = await utils.chainTransactionFee(ethProvider, block.verify_tx_hash);

            senderAccountStat["spent by SENDER ACCOUNT"].eth += verifyTransactionFee;
            senderAccountStat["spent by SENDER ACCOUNT"].usd += verifyTransactionFee * eth_price;

            const transactionUrl = `${providerAddress}/api/v0.1/blocks/${currentBlock}/transactions`;
            const response = await fetch(transactionUrl);
            const transactions = await response.json();

            if (transactions == null) continue;

            for (const transaction of transactions) {
                const transactionTime = new Date(transaction.created_at);

                // TODO: handle fee for `CompleteWithdrawals` operation in L1

                if (utils.correctTransactionWithFee(transaction) && timePeriod.inTime(transactionTime)) {
                    const transactionFee = utils.getTransactionFee(transaction);

                    const tokenID = utils.getTransactionTokenID(transaction);
                    const tokenSymbol = tokensCashed.getTokenSymbol(tokenID);
                    const tokenPrice = tokensCashed.getTokenPrice(tokenSymbol);
                    const tokenAmount = Number(zksProvider.tokenSet.formatToken(tokenSymbol, transactionFee));

                    tokensStat[tokenSymbol].amount += tokenAmount;
                    tokensStat[tokenSymbol].usd += tokenAmount * tokenPrice;
                    tokensStat[tokenSymbol].eth += (tokenAmount * tokenPrice) / eth_price;

                    tokensStat.total.usd += tokenAmount * tokenPrice;
                    tokensStat.total.eth += (tokenAmount * tokenPrice) / eth_price;
                }
            }
        }
    }

    return Object.assign(senderAccountStat, { "collected fees": tokensStat });
}

export async function collectedTokenLiquidations(
    network: Network,
    operatorAddress: string,
    timePeriod: utils.TimePeriod,
    etherscan_api_key: string
) {
    const ethProvider = new ethers.providers.EtherscanProvider(network, etherscan_api_key);

    let liquidationInfo = { "Total amount of ETH": 0 };
    let history: ethers.ethers.providers.TransactionResponse[];

    do {
        const { startBlock, endBlock } = await utils.getBlockInterval(
            ethProvider.baseUrl,
            etherscan_api_key,
            timePeriod
        );

        history = await ethProvider.getHistory(operatorAddress, startBlock, endBlock);

        for (const transaction of history) {
            console.log(`Tx hash: ${transaction.hash}`);

            timePeriod.timeFrom = new Date(transaction.timestamp * 1000 + 1000);
            if (transaction.from == null || transaction.from.toLocaleLowerCase() != operatorAddress) continue;

            const transactionValueWei = transaction.value;
            const transactionValue = Number(ethers.utils.formatEther(transactionValueWei));

            liquidationInfo["Total amount of ETH"] += transactionValue;
        }
    } while (history.length > 0 && timePeriod.isCorrect());

    return liquidationInfo;
}
