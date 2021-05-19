import Axios from 'axios';
import {
    Network,
    TxEthSignature,
    ApiResponse,
    PaginationQuery,
    Paginated,
    ApiBlockInfo,
    ApiAccountInfo,
    Address
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
        if(response.status === 'success') {
            return response.result;
        } else {
            throw new Error(
                `zkSync API response error: errorType: ${response.error.error_type}; code ${response.error.code}; message: ${response.error.message}`,
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

    async accountInfoDetailed(id_or_address: number | Address, info_type: 'committed' | 'finalized'): Promise<ApiResponse> {
        return await this.get(`${this.address}/account/${id_or_address}/${info_type}`);
    }

    async accountInfo(id_or_address: number | Address, info_type: 'committed' | 'finalized'): Promise<ApiAccountInfo> {
        return this.parse_response(await this.accountInfoDetailed(id_or_address, info_type));
    }

    async accountTxsDetailed(id_or_address: number | Address): Promise<ApiResponse> {
        return await this.get(`${this.address}/account/${id_or_address}/transactions`);
    }

    async accountTxs(id_or_address: number | Address): Promise<ApiResponse> {
        return this.parse_response(await this.accountTxsDetailed(id_or_address));
    }

    async accountPendingTxsDetailed(id_or_address: number | Address): Promise<ApiResponse> {
        return await this.get(`${this.address}/account/${id_or_address}/transactions/pending`);
    }

    async accountPendingTxs(id_or_address: number | Address): Promise<ApiResponse> {
        return this.parse_response(await this.accountTxsDetailed(id_or_address));
    }

    async blockPaginationDetailed(pagination_query: PaginationQuery): Promise<ApiResponse> {
        return await this.get(`${this.address}/block?from=${pagination_query.from}&limit=${pagination_query.limit}&direction=${pagination_query.direction}`);
    }

    async blockPagination(pagination_query: PaginationQuery): Promise<Paginated> {
        return this.parse_response(await this.blockPaginationDetailed(pagination_query));
    }

    async blockByPositionDetailed(block_position: number | 'lastCommitted' | 'lastFinalized'): Promise<ApiResponse> {
        return await this.get(`${this.address}/block/${block_position}`);
    }

    async blockByPosition(block_position: number | 'lastCommitted' | 'lastFinalized'): Promise<ApiBlockInfo | null> {
        return this.parse_response(await this.blockByPositionDetailed(block_position));
    }

    async blockTransactionsDetailed(block_position: number | 'lastCommitted' | 'lastFinalized'): Promise<ApiResponse> {
        return await this.get(`${this.address}/block/${block_position}/transaction`);
    }

    async blockTransactions(block_position: number | 'lastCommitted' | 'lastFinalized'): Promise<Paginated> {
        return this.parse_response(await this.blockTransactionsDetailed(block_position));
    }

    async submitTxDetailed(tx: any, signature?: TxEthSignature): Promise<ApiResponse> {
        return await this.post(this.address + '/transaction', {tx, signature});
    }

    async submitTx(tx: any, signature?: TxEthSignature): Promise<String> {
        return this.parse_response(await this.submitTxDetailed(tx, signature));
    }
}
