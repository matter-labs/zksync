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
    L2Tx
} from './types';

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
    public constructor(public address: string) {}

    parse_response(response: ApiResponse): any {
        if (response.status === 'success') {
            return response.result;
        } else {
            throw new Error(
                `zkSync API response error: errorType: ${response.error.error_type}; code ${response.error.code}; message: ${response.error.message}`
            );
        }
    }

    async get(url: string): Promise<ApiResponse> {
        return await Axios.get(url).then((resp) => {
            return resp.data;
        });
    }

    async post(url: string, body: any): Promise<ApiResponse> {
        return await Axios.post(url, body).then((resp) => {
            return resp.data;
        });
    }

    async accountInfoDetailed(
        id_or_address: number | Address,
        info_type: 'committed' | 'finalized'
    ): Promise<ApiResponse> {
        return await this.get(`${this.address}/accounts/${id_or_address}/${info_type}`);
    }

    async accountInfo(
        id_or_address: number | Address,
        info_type: 'committed' | 'finalized'
    ): Promise<ApiAccountInfo | null> {
        return this.parse_response(await this.accountInfoDetailed(id_or_address, info_type));
    }

    async accountTxsDetailed(id_or_address: number | Address, pagination_query: PaginationQuery): Promise<ApiResponse> {
        return await this.get(
            `${this.address}/accounts/${id_or_address}/transactions?from=${pagination_query.from}&limit=${pagination_query.limit}&direction=${pagination_query.direction}`
        );
    }

    async accountTxs(id_or_address: number | Address, pagination_query: PaginationQuery): Promise<Paginated> {
        return this.parse_response(await this.accountTxsDetailed(id_or_address, pagination_query));
    }

    async accountPendingTxsDetailed(
        id_or_address: number | Address,
        pagination_query: PaginationQuery
    ): Promise<ApiResponse> {
        return await this.get(
            `${this.address}/accounts/${id_or_address}/transactions/pending?from=${pagination_query.from}&limit=${pagination_query.limit}&direction=${pagination_query.direction}`
        );
    }

    async accountPendingTxs(id_or_address: number | Address, pagination_query: PaginationQuery): Promise<Paginated> {
        return this.parse_response(await this.accountTxsDetailed(id_or_address, pagination_query));
    }

    async blockPaginationDetailed(pagination_query: PaginationQuery): Promise<ApiResponse> {
        return await this.get(
            `${this.address}/blocks?from=${pagination_query.from}&limit=${pagination_query.limit}&direction=${pagination_query.direction}`
        );
    }

    async blockPagination(pagination_query: PaginationQuery): Promise<Paginated> {
        return this.parse_response(await this.blockPaginationDetailed(pagination_query));
    }

    async blockByPositionDetailed(block_position: number | 'lastCommitted' | 'lastFinalized'): Promise<ApiResponse> {
        return await this.get(`${this.address}/blocks/${block_position}`);
    }

    async blockByPosition(block_position: number | 'lastCommitted' | 'lastFinalized'): Promise<ApiBlockInfo | null> {
        return this.parse_response(await this.blockByPositionDetailed(block_position));
    }

    async blockTransactionsDetailed(
        block_position: number | 'lastCommitted' | 'lastFinalized',
        pagination_query: PaginationQuery
    ): Promise<ApiResponse> {
        return await this.get(
            `${this.address}/blocks/${block_position}/transactions?from=${pagination_query.from}&limit=${pagination_query.limit}&direction=${pagination_query.direction}`
        );
    }

    async blockTransactions(
        block_position: number | 'lastCommitted' | 'lastFinalized',
        pagination_query: PaginationQuery
    ): Promise<Paginated> {
        return this.parse_response(await this.blockTransactionsDetailed(block_position, pagination_query));
    }

    async configDetailed(): Promise<ApiResponse> {
        return await this.get(`${this.address}/config`);
    }

    async config(): Promise<ApiConfig> {
        return this.parse_response(await this.configDetailed());
    }

    async getTxFeeDetailed(
        txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee,
        address: Address,
        tokenLike: TokenLike
    ): Promise<ApiResponse> {
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
        transactions: [
            {
                txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee;
                address: Address;
            }
        ],
        tokenLike: TokenLike
    ): Promise<ApiResponse> {
        return await this.post(`${this.address}/fee/batch`, { transactions, tokenLike });
    }

    async getBatchFee(
        transactions: [
            {
                txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee;
                address: Address;
            }
        ],
        tokenLike: TokenLike
    ): Promise<ApiFee> {
        return this.parse_response(await this.getBatchFeeDetailed(transactions, tokenLike));
    }

    async networkStatusDetailed(): Promise<ApiResponse> {
        return await this.get(`${this.address}/networkStatus`);
    }

    async networkStatus(): Promise<NetworkStatus> {
        return this.parse_response(await this.networkStatusDetailed());
    }

    async tokenPaginationDetailed(pagination_query: PaginationQuery): Promise<ApiResponse> {
        return await this.get(
            `${this.address}/tokens?from=${pagination_query.from}&limit=${pagination_query.limit}&direction=${pagination_query.direction}`
        );
    }

    async tokenPagination(pagination_query: PaginationQuery): Promise<Paginated> {
        return this.parse_response(await this.tokenPaginationDetailed(pagination_query));
    }

    async tokenByIdOrAddressDetailed(id_or_address: number | TokenAddress): Promise<ApiResponse> {
        return await this.get(`${this.address}/tokens/${id_or_address}`);
    }

    async tokenByIdOrAddress(id_or_address: number | TokenAddress): Promise<TokenInfo> {
        return this.parse_response(await this.tokenByIdOrAddressDetailed(id_or_address));
    }

    async tokenPriceDetailed(
        id_or_address: number | TokenAddress,
        token_id_or_usd: number | 'usd'
    ): Promise<ApiResponse> {
        return await this.get(`${this.address}/tokens/${id_or_address}/priceIn/${token_id_or_usd}`);
    }

    async tokenPrice(id_or_address: number | TokenAddress, token_id_or_usd: number | 'usd'): Promise<TokenPriceInfo> {
        return this.parse_response(await this.tokenPriceDetailed(id_or_address, token_id_or_usd));
    }

    async submitTxDetailed(tx: L2Tx, signature?: TxEthSignature): Promise<ApiResponse> {
        return await this.post(`${this.address}/transactions`, { tx, signature });
    }

    async submitTx(tx: L2Tx, signature?: TxEthSignature): Promise<String> {
        return this.parse_response(await this.submitTxDetailed(tx, signature));
    }

    async txStatusDetailed(txHash: string): Promise<ApiResponse> {
        return await this.get(`${this.address}/transactions/${txHash}`);
    }

    async txStatus(txHash: string): Promise<ApiTxReceipt> {
        return this.parse_response(await this.txStatusDetailed(txHash));
    }

    async txDataDetailed(txHash: string): Promise<ApiResponse> {
        return await this.get(`${this.address}/transactions/${txHash}`);
    }

    async txData(txHash: string): Promise<ApiTxAndSignature> {
        return this.parse_response(await this.txDataDetailed(txHash));
    }

    async submitBatchDetailed(txs: [L2Tx], signature: TxEthSignature | TxEthSignature[]): Promise<ApiResponse> {
        return await this.post(`${this.address}/transactions/batches`, { txs, signature });
    }

    async submitBatch(txs: [L2Tx], signature: TxEthSignature | TxEthSignature[]): Promise<SubmitBatchResponse> {
        return this.parse_response(await this.submitBatchDetailed(txs, signature));
    }

    async getBatchDetailed(batchHash: string): Promise<ApiResponse> {
        return await this.get(`${this.address}/transactions/batches/${batchHash}`);
    }

    async getBatch(batchHash: string): Promise<ApiBatchData> {
        return this.parse_response(await this.getBatchDetailed(batchHash));
    }
}
