import Axios from 'axios';
import { BigNumber } from 'ethers';
import { SyncProvider } from './provider-interface';
import * as types from './types';
import { sleep, TokenSet } from './utils';
import { Network } from './types';

export async function getDefaultRestProvider(
    network: types.Network,
    pollIntervalMilliSecs?: number
): Promise<RestProvider> {
    if (network === 'localhost') {
        return await RestProvider.newProvider('http://127.0.0.1:3001/api/v0.2', pollIntervalMilliSecs, network);
    } else if (network === 'goerli') {
        return await RestProvider.newProvider('https://goerli-api.zksync.io/api/v0.2', pollIntervalMilliSecs, network);
    } else if (network === 'sepolia') {
        return await RestProvider.newProvider('https://sepolia-api.zksync.io/api/v0.2', pollIntervalMilliSecs, network);
    } else if (network === 'goerli-beta') {
        return await RestProvider.newProvider(
            'https://goerli-beta-api.zksync.dev/api/v0.2',
            pollIntervalMilliSecs,
            network
        );
    } else if (network === 'rinkeby-beta') {
        return await RestProvider.newProvider(
            'https://rinkeby-beta-api.zksync.io/api/v0.2',
            pollIntervalMilliSecs,
            network
        );
    } else if (network === 'mainnet') {
        return await RestProvider.newProvider('https://api.zksync.io/api/v0.2', pollIntervalMilliSecs, network);
    } else {
        throw new Error(`Ethereum network ${network} is not supported`);
    }
}

export interface Request {
    network: types.Network;
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
        address: string = 'http://127.0.0.1:3001/api/v0.2',
        pollIntervalMilliSecs?: number,
        network?: Network
    ): Promise<RestProvider> {
        const provider = new RestProvider(address);
        if (pollIntervalMilliSecs) {
            provider.pollIntervalMilliSecs = pollIntervalMilliSecs;
        }
        provider.contractAddress = await provider.getContractAddress();
        provider.tokenSet = new TokenSet(await provider.getTokens());
        provider.network = network;
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
        idOrAddress: number | types.Address,
        infoType: 'committed' | 'finalized'
    ): Promise<Response<types.ApiAccountInfo>> {
        return await this.get(`${this.address}/accounts/${idOrAddress}/${infoType}`);
    }

    async accountInfo(
        idOrAddress: number | types.Address,
        infoType: 'committed' | 'finalized'
    ): Promise<types.ApiAccountInfo> {
        return this.parseResponse(await this.accountInfoDetailed(idOrAddress, infoType));
    }

    async toggle2FADetailed(data: types.Toggle2FARequest): Promise<Response<types.Toggle2FAResponse>> {
        return await this.post(`${this.address}/transactions/toggle2FA`, data);
    }

    async toggle2FA(data: types.Toggle2FARequest): Promise<boolean> {
        const response = this.parseResponse(await this.toggle2FADetailed(data));
        return response.success;
    }

    async accountFullInfoDetailed(idOrAddress: number | types.Address): Promise<Response<types.ApiAccountFullInfo>> {
        return await this.get(`${this.address}/accounts/${idOrAddress}`);
    }

    async accountFullInfo(idOrAddress: number | types.Address): Promise<types.ApiAccountFullInfo> {
        return this.parseResponse(await this.accountFullInfoDetailed(idOrAddress));
    }

    async accountTxsDetailed(
        idOrAddress: number | types.Address,
        paginationQuery: types.PaginationQuery<string>,
        token?: types.TokenLike,
        secondIdOrAddress?: number | types.Address
    ): Promise<Response<types.Paginated<types.ApiTransaction, string>>> {
        let url =
            `${this.address}/accounts/${idOrAddress}/transactions?from=${paginationQuery.from}` +
            `&limit=${paginationQuery.limit}&direction=${paginationQuery.direction}`;
        if (token) url += `&token=${token}`;
        if (secondIdOrAddress) url += `&secondAccount=${secondIdOrAddress}`;
        return await this.get(url);
    }

    async accountTxs(
        idOrAddress: number | types.Address,
        paginationQuery: types.PaginationQuery<string>,
        token?: types.TokenLike,
        secondIdOrAddress?: number | types.Address
    ): Promise<types.Paginated<types.ApiTransaction, string>> {
        return this.parseResponse(
            await this.accountTxsDetailed(idOrAddress, paginationQuery, token, secondIdOrAddress)
        );
    }

    async accountPendingTxsDetailed(
        idOrAddress: number | types.Address,
        paginationQuery: types.PaginationQuery<number>
    ): Promise<Response<types.Paginated<types.ApiTransaction, number>>> {
        return await this.get(
            `${this.address}/accounts/${idOrAddress}/transactions/pending?from=${paginationQuery.from}` +
                `&limit=${paginationQuery.limit}&direction=${paginationQuery.direction}`
        );
    }

    async accountPendingTxs(
        idOrAddress: number | types.Address,
        paginationQuery: types.PaginationQuery<number>
    ): Promise<types.Paginated<types.ApiTransaction, number>> {
        return this.parseResponse(await this.accountPendingTxsDetailed(idOrAddress, paginationQuery));
    }

    async blockPaginationDetailed(
        paginationQuery: types.PaginationQuery<number>
    ): Promise<Response<types.Paginated<types.ApiBlockInfo, number>>> {
        return await this.get(
            `${this.address}/blocks?from=${paginationQuery.from}&limit=${paginationQuery.limit}` +
                `&direction=${paginationQuery.direction}`
        );
    }

    async blockPagination(
        paginationQuery: types.PaginationQuery<number>
    ): Promise<types.Paginated<types.ApiBlockInfo, number>> {
        return this.parseResponse(await this.blockPaginationDetailed(paginationQuery));
    }

    async blockByPositionDetailed(blockPosition: types.BlockPosition): Promise<Response<types.ApiBlockInfo>> {
        return await this.get(`${this.address}/blocks/${blockPosition}`);
    }

    async blockByPosition(blockPosition: types.BlockPosition): Promise<types.ApiBlockInfo> {
        return this.parseResponse(await this.blockByPositionDetailed(blockPosition));
    }

    async blockTransactionsDetailed(
        blockPosition: types.BlockPosition,
        paginationQuery: types.PaginationQuery<string>
    ): Promise<Response<types.Paginated<types.ApiTransaction, string>>> {
        return await this.get(
            `${this.address}/blocks/${blockPosition}/transactions?from=${paginationQuery.from}` +
                `&limit=${paginationQuery.limit}&direction=${paginationQuery.direction}`
        );
    }

    async blockTransactions(
        blockPosition: types.BlockPosition,
        paginationQuery: types.PaginationQuery<string>
    ): Promise<types.Paginated<types.ApiTransaction, string>> {
        return this.parseResponse(await this.blockTransactionsDetailed(blockPosition, paginationQuery));
    }

    async configDetailed(): Promise<Response<types.ApiConfig>> {
        return await this.get(`${this.address}/config`);
    }

    async config(): Promise<types.ApiConfig> {
        return this.parseResponse(await this.configDetailed());
    }

    async getTransactionFeeDetailed(
        txType: types.IncomingTxFeeType,
        address: types.Address,
        tokenLike: types.TokenLike
    ): Promise<Response<types.FeeRest>> {
        const rawFee = await this.post<{ gasFee: string; zkpFee: string; totalFee: string }>(`${this.address}/fee`, {
            txType,
            address,
            tokenLike
        });
        let fee: Response<types.FeeRest>;
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
        txType: types.IncomingTxFeeType,
        address: types.Address,
        tokenLike: types.TokenLike
    ): Promise<types.FeeRest> {
        return this.parseResponse(await this.getTransactionFeeDetailed(txType, address, tokenLike));
    }

    async getBatchFullFeeDetailed(
        transactions: {
            txType: types.IncomingTxFeeType;
            address: types.Address;
        }[],
        tokenLike: types.TokenLike
    ): Promise<Response<types.FeeRest>> {
        const rawFee = await this.post<{ gasFee: string; zkpFee: string; totalFee: string }>(
            `${this.address}/fee/batch`,
            { transactions, tokenLike }
        );
        let fee: Response<types.FeeRest>;
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
            txType: types.IncomingTxFeeType;
            address: types.Address;
        }[],
        tokenLike: types.TokenLike
    ): Promise<types.FeeRest> {
        return this.parseResponse(await this.getBatchFullFeeDetailed(transactions, tokenLike));
    }

    async networkStatusDetailed(): Promise<Response<types.NetworkStatus>> {
        return await this.get(`${this.address}/networkStatus`);
    }

    async networkStatus(): Promise<types.NetworkStatus> {
        return this.parseResponse(await this.networkStatusDetailed());
    }

    async tokenPaginationDetailed(
        paginationQuery: types.PaginationQuery<number>
    ): Promise<Response<types.Paginated<types.TokenInfo, number>>> {
        return await this.get(
            `${this.address}/tokens?from=${paginationQuery.from}&limit=${paginationQuery.limit}` +
                `&direction=${paginationQuery.direction}`
        );
    }

    async tokenPagination(
        paginationQuery: types.PaginationQuery<number>
    ): Promise<types.Paginated<types.TokenInfo, number>> {
        return this.parseResponse(await this.tokenPaginationDetailed(paginationQuery));
    }

    async tokenInfoDetailed(tokenLike: types.TokenLike): Promise<Response<types.TokenInfo>> {
        return await this.get(`${this.address}/tokens/${tokenLike}`);
    }

    async tokenInfo(tokenLike: types.TokenLike): Promise<types.TokenInfo> {
        return this.parseResponse(await this.tokenInfoDetailed(tokenLike));
    }

    async tokenPriceInfoDetailed(
        tokenLike: types.TokenLike,
        tokenIdOrUsd: number | 'usd'
    ): Promise<Response<types.TokenPriceInfo>> {
        return await this.get(`${this.address}/tokens/${tokenLike}/priceIn/${tokenIdOrUsd}`);
    }

    async tokenPriceInfo(tokenLike: types.TokenLike, tokenIdOrUsd: number | 'usd'): Promise<types.TokenPriceInfo> {
        return this.parseResponse(await this.tokenPriceInfoDetailed(tokenLike, tokenIdOrUsd));
    }

    async submitTxNewDetailed(tx: types.L2Tx, signature?: types.TxEthSignatureVariant): Promise<Response<string>> {
        return await this.post(`${this.address}/transactions`, { tx, signature });
    }

    async submitTxNew(tx: types.L2Tx, signature?: types.TxEthSignatureVariant): Promise<string> {
        return this.parseResponse(await this.submitTxNewDetailed(tx, signature));
    }

    /**
     * @deprecated Use submitTxNew method instead
     */
    async submitTx(tx: any, signature?: types.TxEthSignatureVariant, fastProcessing?: boolean): Promise<string> {
        if (fastProcessing) {
            tx.fastProcessing = fastProcessing;
        }
        let txHash = await this.submitTxNew(tx, signature);
        txHash.replace('0x', 'sync-tx:');
        return txHash;
    }

    async txStatusDetailed(txHash: string): Promise<Response<types.ApiTxReceipt>> {
        return await this.get(`${this.address}/transactions/${txHash}`);
    }

    async txStatus(txHash: string): Promise<types.ApiTxReceipt> {
        return this.parseResponse(await this.txStatusDetailed(txHash));
    }

    async txDataDetailed(txHash: string): Promise<Response<types.ApiSignedTx>> {
        return await this.get(`${this.address}/transactions/${txHash}/data`);
    }

    async txData(txHash: string): Promise<types.ApiSignedTx> {
        return this.parseResponse(await this.txDataDetailed(txHash));
    }

    async submitTxsBatchNewDetailed(
        txs: { tx: any; signature?: types.TxEthSignatureVariant }[],
        signature?: types.TxEthSignature | types.TxEthSignature[]
    ): Promise<Response<types.SubmitBatchResponse>> {
        return await this.post(`${this.address}/transactions/batches`, { txs, signature });
    }

    async submitTxsBatchNew(
        txs: { tx: any; signature?: types.TxEthSignatureVariant }[],
        signature?: types.TxEthSignature | types.TxEthSignature[]
    ): Promise<types.SubmitBatchResponse> {
        return this.parseResponse(await this.submitTxsBatchNewDetailed(txs, signature));
    }

    /**
     * @deprecated Use submitTxsBatchNew method instead.
     */
    async submitTxsBatch(
        transactions: { tx: any; signature?: types.TxEthSignatureVariant }[],
        ethSignatures?: types.TxEthSignature | types.TxEthSignature[]
    ): Promise<string[]> {
        return (await this.submitTxsBatchNew(transactions, ethSignatures)).transactionHashes;
    }

    async getBatchDetailed(batchHash: string): Promise<Response<types.ApiBatchData>> {
        return await this.get(`${this.address}/transactions/batches/${batchHash}`);
    }

    async getBatch(batchHash: string): Promise<types.ApiBatchData> {
        return this.parseResponse(await this.getBatchDetailed(batchHash));
    }

    async getNFTDetailed(id: number): Promise<Response<types.NFTInfo>> {
        return await this.get(`${this.address}/tokens/nft/${id}`);
    }

    async getNFT(id: number): Promise<types.NFTInfo> {
        const nft = this.parseResponse(await this.getNFTDetailed(id));

        // If the NFT does not exist, throw an exception
        if (nft == null) {
            throw new Error(`Requested NFT doesn't exist or the corresponding mintNFT operation is not verified yet`);
        }
        return nft;
    }

    async getNFTOwnerDetailed(id: number): Promise<Response<number>> {
        return await this.get(`${this.address}/tokens/nft/${id}/owner`);
    }

    async getNFTOwner(id: number): Promise<number> {
        return this.parseResponse(await this.getNFTOwnerDetailed(id));
    }

    async getNFTIdByTxHashDetailed(txHash: string): Promise<Response<number>> {
        return await this.get(`${this.address}/tokens/nft_id_by_tx_hash/${txHash}`);
    }

    async getNFTIdByTxHash(txHash: string): Promise<number> {
        return this.parseResponse(await this.getNFTIdByTxHashDetailed(txHash));
    }

    async notifyAnyTransaction(hash: string, action: 'COMMIT' | 'VERIFY'): Promise<types.ApiTxReceipt> {
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

    async notifyTransaction(hash: string, action: 'COMMIT' | 'VERIFY'): Promise<types.TransactionReceipt> {
        await this.notifyAnyTransaction(hash, action);
        return await this.getTxReceipt(hash);
    }

    async notifyPriorityOp(hash: string, action: 'COMMIT' | 'VERIFY'): Promise<types.PriorityOperationReceipt> {
        await this.notifyAnyTransaction(hash, action);
        return await this.getPriorityOpStatus(hash);
    }

    async getContractAddress(): Promise<types.ContractAddress> {
        const config = await this.config();
        return {
            mainContract: config.contract,
            govContract: config.govContract
        };
    }

    async getTokens(limit?: number): Promise<types.ExtendedTokens> {
        let tokens = {};
        let tmpId = 0;
        limit = limit ? limit : RestProvider.MAX_LIMIT;
        let tokenPage: types.Paginated<types.TokenInfo, number>;
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
                    decimals: token.decimals,
                    enabledForFees: token.enabledForFees
                };
            }
            tmpId += limit;
        } while (tokenPage.list.length == limit);

        return tokens;
    }

    async getState(address: types.Address): Promise<types.AccountState> {
        const fullInfo = await this.accountFullInfo(address);
        const defaultInfo = {
            balances: {},
            nonce: 0,
            pubKeyHash: 'sync:0000000000000000000000000000000000000000',
            nfts: {},
            mintedNfts: {}
        };

        if (fullInfo.finalized) {
            return {
                address,
                id: fullInfo.committed.accountId,
                accountType: fullInfo.committed.accountType,
                depositing: fullInfo.depositing,
                committed: {
                    balances: fullInfo.committed.balances,
                    nonce: fullInfo.committed.nonce,
                    pubKeyHash: fullInfo.committed.pubKeyHash,
                    nfts: fullInfo.committed.nfts,
                    mintedNfts: fullInfo.committed.mintedNfts
                },
                verified: {
                    balances: fullInfo.finalized.balances,
                    nonce: fullInfo.finalized.nonce,
                    pubKeyHash: fullInfo.finalized.pubKeyHash,
                    nfts: fullInfo.finalized.nfts,
                    mintedNfts: fullInfo.finalized.mintedNfts
                }
            };
        } else if (fullInfo.committed) {
            return {
                address,
                id: fullInfo.committed.accountId,
                accountType: fullInfo.committed.accountType,
                depositing: fullInfo.depositing,
                committed: {
                    balances: fullInfo.committed.balances,
                    nonce: fullInfo.committed.nonce,
                    pubKeyHash: fullInfo.committed.pubKeyHash,
                    nfts: fullInfo.committed.nfts,
                    mintedNfts: fullInfo.committed.mintedNfts
                },
                verified: defaultInfo
            };
        } else {
            return {
                address,
                depositing: fullInfo.depositing,
                committed: defaultInfo,
                verified: defaultInfo
            };
        }
    }

    async getConfirmationsForEthOpAmount(): Promise<number> {
        const config = await this.config();
        return config.depositConfirmations;
    }

    async getTransactionsBatchFee(
        txTypes: types.IncomingTxFeeType[],
        addresses: types.Address[],
        tokenLike: types.TokenLike
    ): Promise<BigNumber> {
        let transactions = [];
        for (let i = 0; i < txTypes.length; ++i) {
            transactions.push({ txType: txTypes[i], address: addresses[i] });
        }
        const fee = await this.getBatchFullFee(transactions, tokenLike);
        return fee.totalFee;
    }

    async getTokenPrice(tokenLike: types.TokenLike): Promise<number> {
        const price = await this.tokenPriceInfo(tokenLike, 'usd');
        return parseFloat(price.price);
    }

    async getTxReceipt(txHash: string): Promise<types.TransactionReceipt> {
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

    async getPriorityOpStatus(hash: string): Promise<types.PriorityOperationReceipt> {
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
        if (
            txData.tx.op.type === 'Withdraw' ||
            txData.tx.op.type === 'ForcedExit' ||
            txData.tx.op.type === 'WithdrawNFT'
        ) {
            return txData.tx.op.ethTxHash;
        } else {
            return null;
        }
    }
}
