import Axios from 'axios';
import { BigNumber } from 'ethers';
import { SyncProvider } from './provider-interface';
import {
    Network,
    TokenLike,
    TxEthSignature,
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
    ApiSignedTx,
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

export interface Request {
    network: Network;
    apiVersion: 'v02';
    resource: string;
    args: any;
    timestamp: string;
}

export interface Error {
    errorType: string;
    code: number;
    message: string;
}

export interface Response<T> {
    request: Request;
    status: 'success' | 'error';
    error?: Error;
    result?: T;
}

export class RESTError extends Error {
    constructor(message: string, public restError: Error) {
        super(message);
    }
}

export class RestProvider extends SyncProvider {
    public static readonly MAX_LIMIT = 100;

    private constructor(public address: string) {
        super();
        this.providerType = 'Rest';
    }

    static async newProvider(
        address: string = 'http://127.0.0.1:3001',
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

    parseResponse<T>(response: Response<T>): T {
        if (response.status === 'success') {
            return response.result;
        } else {
            throw new RESTError(
                `zkSync API response error: errorType: ${response.error.errorType};` +
                    ` code ${response.error.code}; message: ${response.error.message}`,
                response.error
            );
        }
    }

    async get<T>(url: string): Promise<Response<T>> {
        return await Axios.get(url).then((resp) => {
            return resp.data;
        });
    }

    async post<T>(url: string, body: any): Promise<Response<T>> {
        return await Axios.post(url, body).then((resp) => {
            return resp.data;
        });
    }

    async accountInfoDetailed(
        idOrAddress: number | Address,
        infoType: 'committed' | 'finalized'
    ): Promise<Response<ApiAccountInfo>> {
        return await this.get(`${this.address}/accounts/${idOrAddress}/${infoType}`);
    }

    async accountInfo(idOrAddress: number | Address, infoType: 'committed' | 'finalized'): Promise<ApiAccountInfo> {
        return this.parseResponse(await this.accountInfoDetailed(idOrAddress, infoType));
    }

    async accountTxsDetailed(
        idOrAddress: number | Address,
        paginationQuery: PaginationQuery<string>
    ): Promise<Response<Paginated<ApiTransaction, AccountTxsRequest>>> {
        return await this.get(
            `${this.address}/accounts/${idOrAddress}/transactions?from=${paginationQuery.from}` +
                `&limit=${paginationQuery.limit}&direction=${paginationQuery.direction}`
        );
    }

    async accountTxs(
        idOrAddress: number | Address,
        paginationQuery: PaginationQuery<string>
    ): Promise<Paginated<ApiTransaction, AccountTxsRequest>> {
        return this.parseResponse(await this.accountTxsDetailed(idOrAddress, paginationQuery));
    }

    async accountPendingTxsDetailed(
        idOrAddress: number | Address,
        paginationQuery: PaginationQuery<number>
    ): Promise<Response<Paginated<ApiTransaction, PendingOpsRequest>>> {
        return await this.get(
            `${this.address}/accounts/${idOrAddress}/transactions/pending?from=${paginationQuery.from}` +
                `&limit=${paginationQuery.limit}&direction=${paginationQuery.direction}`
        );
    }

    async accountPendingTxs(
        idOrAddress: number | Address,
        paginationQuery: PaginationQuery<number>
    ): Promise<Paginated<ApiTransaction, PendingOpsRequest>> {
        return this.parseResponse(await this.accountPendingTxsDetailed(idOrAddress, paginationQuery));
    }

    async blockPaginationDetailed(
        paginationQuery: PaginationQuery<number>
    ): Promise<Response<Paginated<ApiBlockInfo, number>>> {
        return await this.get(
            `${this.address}/blocks?from=${paginationQuery.from}&limit=${paginationQuery.limit}` +
                `&direction=${paginationQuery.direction}`
        );
    }

    async blockPagination(paginationQuery: PaginationQuery<number>): Promise<Paginated<ApiBlockInfo, number>> {
        return this.parseResponse(await this.blockPaginationDetailed(paginationQuery));
    }

    async blockByPositionDetailed(
        blockPosition: number | 'lastCommitted' | 'lastFinalized'
    ): Promise<Response<ApiBlockInfo>> {
        return await this.get(`${this.address}/blocks/${blockPosition}`);
    }

    async blockByPosition(blockPosition: number | 'lastCommitted' | 'lastFinalized'): Promise<ApiBlockInfo> {
        return this.parseResponse(await this.blockByPositionDetailed(blockPosition));
    }

    async blockTransactionsDetailed(
        blockPosition: number | 'lastCommitted' | 'lastFinalized',
        paginationQuery: PaginationQuery<string>
    ): Promise<Response<Paginated<ApiTransaction, BlockAndTxHash>>> {
        return await this.get(
            `${this.address}/blocks/${blockPosition}/transactions?from=${paginationQuery.from}` +
                `&limit=${paginationQuery.limit}&direction=${paginationQuery.direction}`
        );
    }

    async blockTransactions(
        blockPosition: number | 'lastCommitted' | 'lastFinalized',
        paginationQuery: PaginationQuery<string>
    ): Promise<Paginated<ApiTransaction, BlockAndTxHash>> {
        return this.parseResponse(await this.blockTransactionsDetailed(blockPosition, paginationQuery));
    }

    async configDetailed(): Promise<Response<ApiConfig>> {
        return await this.get(`${this.address}/config`);
    }

    async config(): Promise<ApiConfig> {
        return this.parseResponse(await this.configDetailed());
    }

    async getTransactionFeeDetailed(
        txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee,
        address: Address,
        tokenLike: TokenLike
    ): Promise<Response<ApiFee>> {
        const rawFee = await this.post<{ gasFee: string; zkpFee: string; totalFee: string }>(`${this.address}/fee`, {
            txType,
            address,
            tokenLike
        });
        let fee: Response<ApiFee>;
        if (rawFee.status === 'success') {
            fee = {
                request: rawFee.request,
                status: rawFee.status,
                error: null,
                result: {
                    gasFee: BigNumber.from(rawFee.result.gasFee),
                    zkpFee: BigNumber.from(rawFee.result.zkpFee),
                    totalFee: BigNumber.from(rawFee.result.totalFee)
                }
            };
        } else {
            fee = {
                request: rawFee.request,
                status: rawFee.status,
                error: rawFee.error,
                result: null
            };
        }
        return fee;
    }

    async getTransactionFee(
        txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee,
        address: Address,
        tokenLike: TokenLike
    ): Promise<ApiFee> {
        return this.parseResponse(await this.getTransactionFeeDetailed(txType, address, tokenLike));
    }

    async getBatchFullFeeDetailed(
        transactions: {
            txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee;
            address: Address;
        }[],
        tokenLike: TokenLike
    ): Promise<Response<ApiFee>> {
        const rawFee = await this.post<{ gasFee: string; zkpFee: string; totalFee: string }>(
            `${this.address}/fee/batch`,
            { transactions, tokenLike }
        );
        let fee: Response<ApiFee>;
        if (rawFee.status === 'success') {
            fee = {
                request: rawFee.request,
                status: rawFee.status,
                error: null,
                result: {
                    gasFee: BigNumber.from(rawFee.result.gasFee),
                    zkpFee: BigNumber.from(rawFee.result.zkpFee),
                    totalFee: BigNumber.from(rawFee.result.totalFee)
                }
            };
        } else {
            fee = {
                request: rawFee.request,
                status: rawFee.status,
                error: rawFee.error,
                result: null
            };
        }
        return fee;
    }

    async getBatchFullFee(
        transactions: {
            txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee;
            address: Address;
        }[],
        tokenLike: TokenLike
    ): Promise<ApiFee> {
        return this.parseResponse(await this.getBatchFullFeeDetailed(transactions, tokenLike));
    }

    async networkStatusDetailed(): Promise<Response<NetworkStatus>> {
        return await this.get(`${this.address}/networkStatus`);
    }

    async networkStatus(): Promise<NetworkStatus> {
        return this.parseResponse(await this.networkStatusDetailed());
    }

    async tokenPaginationDetailed(
        paginationQuery: PaginationQuery<number>
    ): Promise<Response<Paginated<TokenInfo, number>>> {
        return await this.get(
            `${this.address}/tokens?from=${paginationQuery.from}&limit=${paginationQuery.limit}` +
                `&direction=${paginationQuery.direction}`
        );
    }

    async tokenPagination(paginationQuery: PaginationQuery<number>): Promise<Paginated<TokenInfo, number>> {
        return this.parseResponse(await this.tokenPaginationDetailed(paginationQuery));
    }

    async tokenByIdOrAddressDetailed(idOrAddress: number | TokenAddress): Promise<Response<TokenInfo>> {
        return await this.get(`${this.address}/tokens/${idOrAddress}`);
    }

    async tokenByIdOrAddress(idOrAddress: number | TokenAddress): Promise<TokenInfo> {
        return this.parseResponse(await this.tokenByIdOrAddressDetailed(idOrAddress));
    }

    async tokenPriceInfoDetailed(
        idOrAddress: number | TokenAddress,
        tokenIdOrUsd: number | 'usd'
    ): Promise<Response<TokenPriceInfo>> {
        return await this.get(`${this.address}/tokens/${idOrAddress}/priceIn/${tokenIdOrUsd}`);
    }

    async tokenPriceInfo(idOrAddress: number | TokenAddress, tokenIdOrUsd: number | 'usd'): Promise<TokenPriceInfo> {
        return this.parseResponse(await this.tokenPriceInfoDetailed(idOrAddress, tokenIdOrUsd));
    }

    async submitTxNewDetailed(tx: L2Tx, signature?: TxEthSignature): Promise<Response<string>> {
        return await this.post(`${this.address}/transactions`, { tx, signature });
    }

    async submitTxNew(tx: L2Tx, signature?: TxEthSignature): Promise<string> {
        return this.parseResponse(await this.submitTxNewDetailed(tx, signature));
    }

    /**
     * @deprecated Use submitTxNew method instead
     */
    async submitTx(tx: any, signature?: TxEthSignature, fastProcessing?: boolean): Promise<string> {
        if (fastProcessing) {
            tx.fastProcessing = fastProcessing;
        }
        let txHash = await this.submitTxNew(tx, signature);
        txHash.replace('0x', 'sync-tx:');
        return txHash;
    }

    async txStatusDetailed(txHash: string): Promise<Response<ApiTxReceipt>> {
        return await this.get(`${this.address}/transactions/${txHash}`);
    }

    async txStatus(txHash: string): Promise<ApiTxReceipt> {
        return this.parseResponse(await this.txStatusDetailed(txHash));
    }

    async txDataDetailed(txHash: string): Promise<Response<ApiSignedTx>> {
        return await this.get(`${this.address}/transactions/${txHash}/data`);
    }

    async txData(txHash: string): Promise<ApiSignedTx> {
        return this.parseResponse(await this.txDataDetailed(txHash));
    }

    async submitTxsBatchNewDetailed(
        txs: L2Tx[],
        signature: TxEthSignature | TxEthSignature[]
    ): Promise<Response<SubmitBatchResponse>> {
        return await this.post(`${this.address}/transactions/batches`, { txs, signature });
    }

    async submitTxsBatchNew(txs: L2Tx[], signature: TxEthSignature | TxEthSignature[]): Promise<SubmitBatchResponse> {
        return this.parseResponse(await this.submitTxsBatchNewDetailed(txs, signature));
    }

    async submitTxsBatch(
        transactions: { tx: any; signature?: TxEthSignature }[],
        ethSignatures?: TxEthSignature | TxEthSignature[]
    ): Promise<string[]> {
        let txs = [];
        for (const signedTx of transactions) {
            txs.push(signedTx.tx);
        }
        if (!ethSignatures) {
            throw new Error('Batch signature should be provided in API v0.2');
        }
        return (await this.submitTxsBatchNew(txs, ethSignatures)).transactionHashes;
    }

    async getBatchDetailed(batchHash: string): Promise<Response<ApiBatchData>> {
        return await this.get(`${this.address}/transactions/batches/${batchHash}`);
    }

    async getBatch(batchHash: string): Promise<ApiBatchData> {
        return this.parseResponse(await this.getBatchDetailed(batchHash));
    }

    async notifyAnyTransaction(hash: string, action: 'COMMIT' | 'VERIFY'): Promise<ApiTxReceipt> {
        while (true) {
            let transactionStatus = await this.txStatus(hash);
            let notifyDone;
            if (action === 'COMMIT') {
                notifyDone = transactionStatus && transactionStatus.rollupBlock;
            } else {
                if (transactionStatus && transactionStatus.rollupBlock) {
                    if (transactionStatus.status === 'rejected') {
                        // If the transaction status is rejected
                        // it cannot be known if transaction is queued, committed or finalized.
                        // That is why there is separate `blockByPosition` query.
                        const blockStatus = await this.blockByPosition(transactionStatus.rollupBlock);
                        notifyDone = blockStatus && blockStatus.status === 'finalized';
                    } else {
                        notifyDone = transactionStatus.status === 'finalized';
                    }
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

    async getTokens(limit?: number): Promise<Tokens> {
        let tokens = {};
        let tmpId = 0;
        limit = limit ? limit : RestProvider.MAX_LIMIT;
        let tokenPage: Paginated<TokenInfo, number>;
        do {
            tokenPage = await this.tokenPagination({
                from: tmpId,
                limit,
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
            tmpId += limit;
        } while (tokenPage.list.length == limit);

        return tokens;
    }

    async getState(address: Address): Promise<AccountState> {
        const committedFullInfo = await this.accountInfo(address, 'committed');
        const finalizedFullInfo = await this.accountInfo(address, 'finalized');

        if (finalizedFullInfo) {
            return {
                address,
                id: committedFullInfo.accountId,
                committed: {
                    balances: committedFullInfo.balances,
                    nonce: committedFullInfo.nonce,
                    pubKeyHash: committedFullInfo.pubKeyHash
                },
                verified: {
                    balances: finalizedFullInfo.balances,
                    nonce: finalizedFullInfo.nonce,
                    pubKeyHash: finalizedFullInfo.pubKeyHash
                }
            };
        } else if (committedFullInfo) {
            return {
                address,
                id: committedFullInfo.accountId,
                committed: {
                    balances: committedFullInfo.balances,
                    nonce: committedFullInfo.nonce,
                    pubKeyHash: committedFullInfo.pubKeyHash
                },
                verified: {
                    balances: {},
                    nonce: 0,
                    pubKeyHash: 'sync:0000000000000000000000000000000000000000'
                }
            };
        } else {
            return {
                address,
                committed: {
                    balances: {},
                    nonce: 0,
                    pubKeyHash: 'sync:0000000000000000000000000000000000000000'
                },
                verified: {
                    balances: {},
                    nonce: 0,
                    pubKeyHash: 'sync:0000000000000000000000000000000000000000'
                }
            };
        }
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
        if (!receipt || !receipt.rollupBlock) {
            return {
                executed: false
            };
        } else {
            if (receipt.status === 'rejected') {
                const blockFullInfo = await this.blockByPosition(receipt.rollupBlock);
                const blockInfo = {
                    blockNumber: receipt.rollupBlock,
                    committed: blockFullInfo ? true : false,
                    verified: blockFullInfo && blockFullInfo.status === 'finalized' ? true : false
                };
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
                    block: {
                        blockNumber: receipt.rollupBlock,
                        committed: true,
                        verified: receipt.status === 'finalized'
                    }
                };
            }
        }
    }

    async getPriorityOpStatus(hash: string): Promise<PriorityOperationReceipt> {
        const receipt = await this.txStatus(hash);
        if (!receipt || !receipt.rollupBlock) {
            return {
                executed: false
            };
        } else {
            return {
                executed: true,
                block: {
                    blockNumber: receipt.rollupBlock,
                    committed: true,
                    verified: receipt.status === 'finalized'
                }
            };
        }
    }

    async getEthTxForWithdrawal(withdrawalHash: string): Promise<string> {
        const txData = await this.txData(withdrawalHash);
        if (txData.tx.op.type === 'Withdraw' || txData.tx.op.type === 'ForcedExit') {
            return txData.tx.op.ethTxHash;
        } else {
            return null;
        }
    }
}
