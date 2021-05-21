import Axios from 'axios';
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
    AccountTxsRequest
} from './types';
import { sleep } from './utils';

export function getDefaultRestProvider(network: Network): RestProvider {
    if (network === 'localhost') {
        return new RestProvider('http://127.0.0.1:3001/api/v0.2');
    } else if (network === 'ropsten') {
        return new RestProvider('https://ropsten-api.zksync.io/api/v0.2');
    } else if (network === 'rinkeby') {
        return new RestProvider('https://rinkeby-api.zksync.io/api/v0.2');
    } else if (network === 'ropsten-beta') {
        return new RestProvider('https://ropsten-beta-api.zksync.io/api/v0.2');
    } else if (network === 'rinkeby-beta') {
        return new RestProvider('https://rinkeby-beta-api.zksync.io/api/v0.2');
    } else if (network === 'mainnet') {
        return new RestProvider('https://api.zksync.io/api/v0.2');
    } else {
        throw new Error(`Ethereum network ${network} is not supported`);
    }
}

export class RestProvider {
    public pollIntervalMilliSecs = 500;

    public constructor(public address: string) {}

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

    async getTxFeeDetailed(
        txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee,
        address: Address,
        tokenLike: TokenLike
    ): Promise<ApiResponse<ApiFee>> {
        return await this.post(`${this.address}/fee`, { txType, address, tokenLike });
    }

    async getTxFee(
        txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee,
        address: Address,
        tokenLike: TokenLike
    ): Promise<ApiFee> {
        return this.parse_response(await this.getTxFeeDetailed(txType, address, tokenLike));
    }

    async getBatchFeeDetailed(
        transactions: {
            txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee;
            address: Address;
        }[],
        tokenLike: TokenLike
    ): Promise<ApiResponse<ApiFee>> {
        return await this.post(`${this.address}/fee/batch`, { transactions, tokenLike });
    }

    async getBatchFee(
        transactions: {
            txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee;
            address: Address;
        }[],
        tokenLike: TokenLike
    ): Promise<ApiFee> {
        return this.parse_response(await this.getBatchFeeDetailed(transactions, tokenLike));
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

    async tokenPriceDetailed(
        idOrAddress: number | TokenAddress,
        tokenIdOrUsd: number | 'usd'
    ): Promise<ApiResponse<TokenPriceInfo>> {
        return await this.get(`${this.address}/tokens/${idOrAddress}/priceIn/${tokenIdOrUsd}`);
    }

    async tokenPrice(idOrAddress: number | TokenAddress, tokenIdOrUsd: number | 'usd'): Promise<TokenPriceInfo> {
        return this.parse_response(await this.tokenPriceDetailed(idOrAddress, tokenIdOrUsd));
    }

    async submitTxDetailed(tx: L2Tx, signature?: TxEthSignature): Promise<ApiResponse<String>> {
        return await this.post(`${this.address}/transactions`, { tx, signature });
    }

    async submitTx(tx: L2Tx, signature?: TxEthSignature): Promise<String> {
        return this.parse_response(await this.submitTxDetailed(tx, signature));
    }

    async txStatusDetailed(txHash: string): Promise<ApiResponse<ApiTxReceipt>> {
        return await this.get(`${this.address}/transactions/${txHash}`);
    }

    async txStatus(txHash: string): Promise<ApiTxReceipt> {
        return this.parse_response(await this.txStatusDetailed(txHash));
    }

    async txDataDetailed(txHash: string): Promise<ApiResponse<ApiTxAndSignature>> {
        return await this.get(`${this.address}/transactions/${txHash}`);
    }

    async txData(txHash: string): Promise<ApiTxAndSignature> {
        return this.parse_response(await this.txDataDetailed(txHash));
    }

    async submitBatchDetailed(
        txs: L2Tx[],
        signature: TxEthSignature | TxEthSignature[]
    ): Promise<ApiResponse<SubmitBatchResponse>> {
        return await this.post(`${this.address}/transactions/batches`, { txs, signature });
    }

    async submitBatch(txs: L2Tx[], signature: TxEthSignature | TxEthSignature[]): Promise<SubmitBatchResponse> {
        return this.parse_response(await this.submitBatchDetailed(txs, signature));
    }

    async getBatchDetailed(batchHash: string): Promise<ApiResponse<ApiBatchData>> {
        return await this.get(`${this.address}/transactions/batches/${batchHash}`);
    }

    async getBatch(batchHash: string): Promise<ApiBatchData> {
        return this.parse_response(await this.getBatchDetailed(batchHash));
    }

    async notifyTransactionDetailed(
        txHash: string,
        state: 'committed' | 'finalized'
    ): Promise<ApiResponse<ApiTxReceipt>> {
        while (true) {
            let transactionStatus = await this.txStatusDetailed(txHash);
            let notifyDone;
            if (state === 'committed') {
                notifyDone = transactionStatus.result && transactionStatus.result.rollupBlock;
            } else {
                if (transactionStatus.result && transactionStatus.result.rollupBlock) {
                    // If the transaction status is rejected it cannot be known if transaction is queued, committed or finalized.
                    const blockStatus = await this.blockByPositionDetailed(transactionStatus.result.rollupBlock);
                    notifyDone = blockStatus.result && blockStatus.result.status === 'finalized';
                }
            }
            if (notifyDone) {
                // Transaction status needs to be updated if status
                // was updated between `txStatusDetailed` and `blockByPositionDetailed` queries.
                transactionStatus = await this.txStatusDetailed(txHash);
                return transactionStatus;
            } else {
                await sleep(this.pollIntervalMilliSecs);
            }
        }
    }

    async notifyTransaction(txHash: string, state: 'committed' | 'finalized'): Promise<ApiTxReceipt> {
        return this.parse_response(await this.notifyTransactionDetailed(txHash, state));
    }
}
