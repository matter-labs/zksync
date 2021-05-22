import Axios from 'axios';
import { BigNumber } from 'ethers';
import { SyncProvider } from './provider-interface';
import {
    Network,
    TokenLike,
    TxEthSignature,
    ApiResponse,
    PaginationQuery,
    Paginated,
    ApiBlockInfo,
    ApiAccountInfo,
    Address,
    ApiConfig,
    ChangePubKeyFee,
    LegacyChangePubKeyFee,
    ApiFee,
    NetworkStatus,
    TokenAddress,
    TokenInfo,
    TokenPriceInfo,
    SubmitBatchResponse,
    ApiTxReceipt,
    ApiTxAndSignature,
    ApiBatchData,
    L2Tx,
    ApiTransaction,
    BlockAndTxHash,
    PendingOpsRequest,
    AccountTxsRequest,
    ContractAddress,
    Tokens,
    AccountState,
    TransactionReceipt,
    PriorityOperationReceipt
} from './types';
import { sleep, TokenSet } from './utils';

export async function getDefaultRestProvider(network: Network): Promise<RestProvider> {
    if (network === 'localhost') {
        return await RestProvider.newProvider('http://127.0.0.1:3001/api/v0.2');
    } else if (network === 'ropsten') {
        return await RestProvider.newProvider('https://ropsten-api.zksync.io/api/v0.2');
    } else if (network === 'rinkeby') {
        return await RestProvider.newProvider('https://rinkeby-api.zksync.io/api/v0.2');
    } else if (network === 'ropsten-beta') {
        return await RestProvider.newProvider('https://ropsten-beta-api.zksync.io/api/v0.2');
    } else if (network === 'rinkeby-beta') {
        return await RestProvider.newProvider('https://rinkeby-beta-api.zksync.io/api/v0.2');
    } else if (network === 'mainnet') {
        return await RestProvider.newProvider('https://api.zksync.io/api/v0.2');
    } else {
        throw new Error(`Ethereum network ${network} is not supported`);
    }
}

export class RestProvider extends SyncProvider {
    public pollIntervalMilliSecs = 500;

    private constructor(public address: string) {
        super();
        this.providerType = 'Rest';
    }

    static async newProvider(
        address: string = 'http://127.0.0.1:3030',
        pollIntervalMilliSecs?: number
    ): Promise<RestProvider> {
        const provider = new RestProvider(address);
        if (pollIntervalMilliSecs) {
            provider.pollIntervalMilliSecs = pollIntervalMilliSecs;
        }
        provider.contractAddress = await provider.getContractAddress();
        provider.tokenSet = new TokenSet(await provider.getTokens());
        return provider;
    }

    parse_response<T>(response: ApiResponse<T>): T {
        if (response.status === 'success') {
            return response.result;
        } else {
            throw new Error(
                `zkSync API response error: errorType: ${response.error.errorType}; code ${response.error.code}; message: ${response.error.message}`
            );
        }
    }

    async get<T>(url: string): Promise<ApiResponse<T>> {
        return await Axios.get(url).then((resp) => {
            return resp.data;
        });
    }

    async post<T>(url: string, body: any): Promise<ApiResponse<T>> {
        return await Axios.post(url, body).then((resp) => {
            return resp.data;
        });
    }

    async accountInfoDetailed(
        idOrAddress: number | Address,
        infoType: 'committed' | 'finalized'
    ): Promise<ApiResponse<ApiAccountInfo | null>> {
        return await this.get(`${this.address}/accounts/${idOrAddress}/${infoType}`);
    }

    async accountInfo(
        idOrAddress: number | Address,
        infoType: 'committed' | 'finalized'
    ): Promise<ApiAccountInfo | null> {
        return this.parse_response(await this.accountInfoDetailed(idOrAddress, infoType));
    }

    async accountTxsDetailed(
        idOrAddress: number | Address,
        paginationQuery: PaginationQuery<string>
    ): Promise<ApiResponse<Paginated<ApiTransaction, AccountTxsRequest>>> {
        return await this.get(
            `${this.address}/accounts/${idOrAddress}/transactions?from=${paginationQuery.from}&limit=${paginationQuery.limit}&direction=${paginationQuery.direction}`
        );
    }

    async accountTxs(
        idOrAddress: number | Address,
        paginationQuery: PaginationQuery<string>
    ): Promise<Paginated<ApiTransaction, AccountTxsRequest>> {
        return this.parse_response(await this.accountTxsDetailed(idOrAddress, paginationQuery));
    }

    async accountPendingTxsDetailed(
        idOrAddress: number | Address,
        paginationQuery: PaginationQuery<number>
    ): Promise<ApiResponse<Paginated<ApiTransaction, PendingOpsRequest>>> {
        return await this.get(
            `${this.address}/accounts/${idOrAddress}/transactions/pending?from=${paginationQuery.from}&limit=${paginationQuery.limit}&direction=${paginationQuery.direction}`
        );
    }

    async accountPendingTxs(
        idOrAddress: number | Address,
        paginationQuery: PaginationQuery<number>
    ): Promise<Paginated<ApiTransaction, PendingOpsRequest>> {
        return this.parse_response(await this.accountPendingTxsDetailed(idOrAddress, paginationQuery));
    }

    async blockPaginationDetailed(
        paginationQuery: PaginationQuery<number>
    ): Promise<ApiResponse<Paginated<ApiBlockInfo, number>>> {
        return await this.get(
            `${this.address}/blocks?from=${paginationQuery.from}&limit=${paginationQuery.limit}&direction=${paginationQuery.direction}`
        );
    }

    async blockPagination(paginationQuery: PaginationQuery<number>): Promise<Paginated<ApiBlockInfo, number>> {
        return this.parse_response(await this.blockPaginationDetailed(paginationQuery));
    }

    async blockByPositionDetailed(
        blockPosition: number | 'lastCommitted' | 'lastFinalized'
    ): Promise<ApiResponse<ApiBlockInfo | null>> {
        return await this.get(`${this.address}/blocks/${blockPosition}`);
    }

    async blockByPosition(blockPosition: number | 'lastCommitted' | 'lastFinalized'): Promise<ApiBlockInfo | null> {
        return this.parse_response(await this.blockByPositionDetailed(blockPosition));
    }

    async blockTransactionsDetailed(
        blockPosition: number | 'lastCommitted' | 'lastFinalized',
        paginationQuery: PaginationQuery<string>
    ): Promise<ApiResponse<Paginated<ApiTransaction, BlockAndTxHash>>> {
        return await this.get(
            `${this.address}/blocks/${blockPosition}/transactions?from=${paginationQuery.from}&limit=${paginationQuery.limit}&direction=${paginationQuery.direction}`
        );
    }

    async blockTransactions(
        blockPosition: number | 'lastCommitted' | 'lastFinalized',
        paginationQuery: PaginationQuery<string>
    ): Promise<Paginated<ApiTransaction, BlockAndTxHash>> {
        return this.parse_response(await this.blockTransactionsDetailed(blockPosition, paginationQuery));
    }

    async configDetailed(): Promise<ApiResponse<ApiConfig>> {
        return await this.get(`${this.address}/config`);
    }

    async config(): Promise<ApiConfig> {
        return this.parse_response(await this.configDetailed());
    }

    async getTransactionFeeDetailed(
        txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee,
        address: Address,
        tokenLike: TokenLike
    ): Promise<ApiResponse<ApiFee>> {
        return await this.post(`${this.address}/fee`, { txType, address, tokenLike });
    }

    async getTransactionFee(
        txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee,
        address: Address,
        tokenLike: TokenLike
    ): Promise<ApiFee> {
        return this.parse_response(await this.getTransactionFeeDetailed(txType, address, tokenLike));
    }

    async getBatchFullFeeDetailed(
        transactions: {
            txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee;
            address: Address;
        }[],
        tokenLike: TokenLike
    ): Promise<ApiResponse<ApiFee>> {
        return await this.post(`${this.address}/fee/batch`, { transactions, tokenLike });
    }

    async getBatchFullFee(
        transactions: {
            txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee;
            address: Address;
        }[],
        tokenLike: TokenLike
    ): Promise<ApiFee> {
        return this.parse_response(await this.getBatchFullFeeDetailed(transactions, tokenLike));
    }

    async networkStatusDetailed(): Promise<ApiResponse<NetworkStatus>> {
        return await this.get(`${this.address}/networkStatus`);
    }

    async networkStatus(): Promise<NetworkStatus> {
        return this.parse_response(await this.networkStatusDetailed());
    }

    async tokenPaginationDetailed(
        paginationQuery: PaginationQuery<number>
    ): Promise<ApiResponse<Paginated<TokenInfo, number>>> {
        return await this.get(
            `${this.address}/tokens?from=${paginationQuery.from}&limit=${paginationQuery.limit}&direction=${paginationQuery.direction}`
        );
    }

    async tokenPagination(paginationQuery: PaginationQuery<number>): Promise<Paginated<TokenInfo, number>> {
        return this.parse_response(await this.tokenPaginationDetailed(paginationQuery));
    }

    async tokenByIdOrAddressDetailed(idOrAddress: number | TokenAddress): Promise<ApiResponse<TokenInfo>> {
        return await this.get(`${this.address}/tokens/${idOrAddress}`);
    }

    async tokenByIdOrAddress(idOrAddress: number | TokenAddress): Promise<TokenInfo> {
        return this.parse_response(await this.tokenByIdOrAddressDetailed(idOrAddress));
    }

    async tokenPriceInfoDetailed(
        idOrAddress: number | TokenAddress,
        tokenIdOrUsd: number | 'usd'
    ): Promise<ApiResponse<TokenPriceInfo>> {
        return await this.get(`${this.address}/tokens/${idOrAddress}/priceIn/${tokenIdOrUsd}`);
    }

    async tokenPriceInfo(idOrAddress: number | TokenAddress, tokenIdOrUsd: number | 'usd'): Promise<TokenPriceInfo> {
        return this.parse_response(await this.tokenPriceInfoDetailed(idOrAddress, tokenIdOrUsd));
    }

    async submitTxNewDetailed(tx: L2Tx, signature?: TxEthSignature): Promise<ApiResponse<string>> {
        return await this.post(`${this.address}/transactions`, { tx, signature });
    }

    async submitTxNew(tx: L2Tx, signature?: TxEthSignature): Promise<string> {
        return this.parse_response(await this.submitTxNewDetailed(tx, signature));
    }

    async submitTx(tx: any, signature?: TxEthSignature, fastProcessing?: boolean): Promise<string> {
        if (fastProcessing !== undefined) {
            tx.fastProcessing = fastProcessing;
        }
        return await this.submitTxNew(tx, signature);
    }

    async txStatusDetailed(txHash: string): Promise<ApiResponse<ApiTxReceipt | null>> {
        return await this.get(`${this.address}/transactions/${txHash}`);
    }

    async txStatus(txHash: string): Promise<ApiTxReceipt | null> {
        return this.parse_response(await this.txStatusDetailed(txHash));
    }

    async txDataDetailed(txHash: string): Promise<ApiResponse<ApiTxAndSignature | null>> {
        return await this.get(`${this.address}/transactions/${txHash}`);
    }

    async txData(txHash: string): Promise<ApiTxAndSignature | null> {
        return this.parse_response(await this.txDataDetailed(txHash));
    }

    async submitTxsBatchNewDetailed(
        txs: L2Tx[],
        signature: TxEthSignature | TxEthSignature[]
    ): Promise<ApiResponse<SubmitBatchResponse>> {
        return await this.post(`${this.address}/transactions/batches`, { txs, signature });
    }

    async submitTxsBatchNew(txs: L2Tx[], signature: TxEthSignature | TxEthSignature[]): Promise<SubmitBatchResponse> {
        return this.parse_response(await this.submitTxsBatchNewDetailed(txs, signature));
    }

    async submitTxsBatch(
        transactions: { tx: any; signature?: TxEthSignature }[],
        ethSignatures?: TxEthSignature | TxEthSignature[]
    ): Promise<string[]> {
        let txs = [];
        for (const txAndSignature of transactions) {
            txs.push(txAndSignature.tx);
        }
        if (ethSignatures === undefined) {
            throw new Error('Batch signature should be provided in API v0.2');
        }
        return await (await this.submitTxsBatchNew(txs, ethSignatures)).transactionHashes;
    }

    async getBatchDetailed(batchHash: string): Promise<ApiResponse<ApiBatchData>> {
        return await this.get(`${this.address}/transactions/batches/${batchHash}`);
    }

    async getBatch(batchHash: string): Promise<ApiBatchData> {
        return this.parse_response(await this.getBatchDetailed(batchHash));
    }

    async notifyAnyTransaction(hash: string, action: 'COMMIT' | 'VERIFY'): Promise<ApiTxReceipt> {
        while (true) {
            let transactionStatus = await this.txStatus(hash);
            let notifyDone;
            if (action === 'COMMIT') {
                notifyDone = transactionStatus.rollupBlock !== null;
            } else {
                if (transactionStatus.rollupBlock !== null) {
                    // If the transaction status is rejected it cannot be known if transaction is queued, committed or finalized.
                    const blockStatus = await this.blockByPosition(transactionStatus.rollupBlock);
                    notifyDone = blockStatus.status === 'finalized';
                }
            }
            if (notifyDone) {
                // Transaction status needs to be recalculated because it can
                // be updated between `txStatus` and `blockByPosition` calls.
                return await this.txStatus(hash);
            } else {
                await sleep(this.pollIntervalMilliSecs);
            }
        }
    }

    async notifyTransaction(hash: string, action: 'COMMIT' | 'VERIFY'): Promise<TransactionReceipt> {
        await this.notifyAnyTransaction(hash, action);
        return await this.getTxReceipt(hash);
    }

    async notifyPriorityOp(hash: string, action: 'COMMIT' | 'VERIFY'): Promise<PriorityOperationReceipt> {
        await this.notifyAnyTransaction(hash, action);
        return await this.getPriorityOpStatus(hash);
    }

    async getContractAddress(): Promise<ContractAddress> {
        const config = await this.config();
        return {
            mainContract: config.contract,
            govContract: config.govContract
        };
    }

    async getTokens(): Promise<Tokens> {
        let tokens = {};
        let lastId = 0;
        let maxLimit = 100; //TODO
        let tokenPage: Paginated<TokenInfo, number>;
        do {
            tokenPage = await this.tokenPagination({
                from: lastId,
                limit: maxLimit,
                direction: 'newer'
            });
            for (let token of tokenPage.list) {
                tokens[token.symbol] = {
                    address: token.address,
                    id: token.id,
                    symbol: token.symbol,
                    decimals: token.decimals
                };
            }
            lastId += maxLimit;
        } while (tokenPage.list.length == maxLimit);

        return tokens;
    }

    async getState(address: Address): Promise<AccountState> {
        const committed = await this.accountInfo(address, 'committed');
        const finalized = await this.accountInfo(address, 'finalized');
        return {
            address,
            id: committed.accountId,
            committed: {
                balances: committed.balances,
                nonce: committed.nonce,
                pubKeyHash: committed.pubKeyHash
            },
            verified: {
                balances: finalized.balances,
                nonce: finalized.nonce,
                pubKeyHash: finalized.pubKeyHash
            }
        };
    }

    async getConfirmationsForEthOpAmount(): Promise<number> {
        const config = await this.config();
        return config.depositConfirmations;
    }

    async getTransactionsBatchFee(
        txTypes: ('Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee)[],
        addresses: Address[],
        tokenLike: TokenLike
    ): Promise<BigNumber> {
        let transactions = [];
        for (let i = 0; i < txTypes.length; ++i) {
            transactions.push({ txType: txTypes[i], address: addresses[i] });
        }
        const fee = await this.getBatchFullFee(transactions, tokenLike);
        return fee.totalFee;
    }

    async getTokenPrice(tokenLike: TokenLike): Promise<number> {
        const price = await this.tokenPriceInfo(tokenLike, 'usd');
        return price.price.toNumber();
    }

    async getTxReceipt(txHash: string): Promise<TransactionReceipt> {
        const receipt = await this.txStatus(txHash);
        if (receipt === null || receipt.rollupBlock === null) {
            return {
                executed: false
            };
        } else {
            const blockFullInfo = await this.blockByPosition(receipt.rollupBlock);
            const blockInfo = {
                blockNumber: blockFullInfo.blockNumber,
                committed: blockFullInfo.commitTxHash !== null,
                verified: blockFullInfo.verifyTxHash !== null
            };
            if (receipt.status === 'rejected') {
                return {
                    executed: true,
                    success: false,
                    failReason: receipt.failReason,
                    block: blockInfo
                };
            } else {
                return {
                    executed: true,
                    success: true,
                    block: blockInfo
                };
            }
        }
    }

    async getPriorityOpStatus(hash: string): Promise<PriorityOperationReceipt> {
        const receipt = await this.txStatus(hash);
        if (receipt === null || receipt.rollupBlock === null) {
            return {
                executed: false
            };
        } else {
            const blockFullInfo = await this.blockByPosition(receipt.rollupBlock);
            const blockInfo = {
                blockNumber: blockFullInfo.blockNumber,
                committed: blockFullInfo.commitTxHash !== null,
                verified: blockFullInfo.verifyTxHash !== null
            };
            return {
                executed: true,
                block: blockInfo
            };
        }
    }
}
