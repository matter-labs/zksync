import { AbstractJSONRPCTransport, DummyTransport, HTTPTransport, WSTransport } from './transport';
import { BigNumber, ethers } from 'ethers';
import {
    AccountState,
    Address,
    ChangePubKeyFee,
    ContractAddress,
    Fee,
    LegacyChangePubKeyFee,
    Network,
    NFT,
    PriorityOperationReceipt,
    TokenAddress,
    TokenLike,
    Tokens,
    TransactionReceipt,
    TxEthSignature,
    TxEthSignatureVariant
} from './types';
import { isTokenETH, sleep, TokenSet, isNFT } from './utils';
import {
    Governance,
    GovernanceFactory,
    ZkSync,
    ZkSyncFactory,
    ZkSyncNFTFactory,
    ZkSyncNFTFactoryFactory
} from './typechain';

export async function getDefaultProvider(network: Network, transport: 'WS' | 'HTTP' = 'HTTP'): Promise<Provider> {
    if (transport === 'WS') {
        console.warn('Websocket support will be removed in future. Use HTTP transport instead.');
    }
    if (network === 'localhost') {
        if (transport === 'WS') {
            return await Provider.newWebsocketProvider('ws://127.0.0.1:3031');
        } else if (transport === 'HTTP') {
            return await Provider.newHttpProvider('http://127.0.0.1:3030');
        }
    } else if (network === 'ropsten') {
        if (transport === 'WS') {
            return await Provider.newWebsocketProvider('wss://ropsten-api.zksync.io/jsrpc-ws');
        } else if (transport === 'HTTP') {
            return await Provider.newHttpProvider('https://ropsten-api.zksync.io/jsrpc');
        }
    } else if (network === 'rinkeby') {
        if (transport === 'WS') {
            return await Provider.newWebsocketProvider('wss://rinkeby-api.zksync.io/jsrpc-ws');
        } else if (transport === 'HTTP') {
            return await Provider.newHttpProvider('https://rinkeby-api.zksync.io/jsrpc');
        }
    } else if (network === 'ropsten-beta') {
        if (transport === 'WS') {
            return await Provider.newWebsocketProvider('wss://ropsten-beta-api.zksync.io/jsrpc-ws');
        } else if (transport === 'HTTP') {
            return await Provider.newHttpProvider('https://ropsten-beta-api.zksync.io/jsrpc');
        }
    } else if (network === 'rinkeby-beta') {
        if (transport === 'WS') {
            return await Provider.newWebsocketProvider('wss://rinkeby-beta-api.zksync.io/jsrpc-ws');
        } else if (transport === 'HTTP') {
            return await Provider.newHttpProvider('https://rinkeby-beta-api.zksync.io/jsrpc');
        }
    } else if (network === 'mainnet') {
        if (transport === 'WS') {
            return await Provider.newWebsocketProvider('wss://api.zksync.io/jsrpc-ws');
        } else if (transport === 'HTTP') {
            return await Provider.newHttpProvider('https://api.zksync.io/jsrpc');
        }
    } else {
        throw new Error(`Ethereum network ${network} is not supported`);
    }
}

export class Provider {
    contractAddress: ContractAddress;
    public tokenSet: TokenSet;

    // For HTTP provider
    public pollIntervalMilliSecs = 500;

    private constructor(public transport: AbstractJSONRPCTransport) {}

    /**
     * @deprecated Websocket support will be removed in future. Use HTTP transport instead.
     */
    static async newWebsocketProvider(address: string): Promise<Provider> {
        const transport = await WSTransport.connect(address);
        const provider = new Provider(transport);
        provider.contractAddress = await provider.getContractAddress();
        provider.tokenSet = new TokenSet(await provider.getTokens());
        return provider;
    }

    static async newHttpProvider(
        address: string = 'http://127.0.0.1:3030',
        pollIntervalMilliSecs?: number
    ): Promise<Provider> {
        const transport = new HTTPTransport(address);
        const provider = new Provider(transport);
        if (pollIntervalMilliSecs) {
            provider.pollIntervalMilliSecs = pollIntervalMilliSecs;
        }
        provider.contractAddress = await provider.getContractAddress();
        provider.tokenSet = new TokenSet(await provider.getTokens());
        return provider;
    }

    /**
     * Provides some hardcoded values the `Provider` responsible for
     * without communicating with the network
     */
    static async newMockProvider(network: string, ethPrivateKey: Uint8Array, getTokens: Function): Promise<Provider> {
        const transport = new DummyTransport(network, ethPrivateKey, getTokens);
        const provider = new Provider(transport);

        provider.contractAddress = await provider.getContractAddress();
        provider.tokenSet = new TokenSet(await provider.getTokens());
        return provider;
    }

    // return transaction hash (e.g. sync-tx:dead..beef)
    async submitTx(tx: any, signature?: TxEthSignatureVariant, fastProcessing?: boolean): Promise<string> {
        return await this.transport.request('tx_submit', [tx, signature, fastProcessing]);
    }

    // Requests `zkSync` server to execute several transactions together.
    // return transaction hash (e.g. sync-tx:dead..beef)
    async submitTxsBatch(
        transactions: { tx: any; signature?: TxEthSignatureVariant }[],
        ethSignatures?: TxEthSignature | TxEthSignature[]
    ): Promise<string[]> {
        let signatures: TxEthSignature[] = [];
        // For backwards compatibility we allow sending single signature as well
        // as no signatures at all.
        if (ethSignatures == undefined) {
            signatures = [];
        } else if (ethSignatures instanceof Array) {
            signatures = ethSignatures;
        } else {
            signatures.push(ethSignatures);
        }
        return await this.transport.request('submit_txs_batch', [transactions, signatures]);
    }

    async getContractAddress(): Promise<ContractAddress> {
        return await this.transport.request('contract_address', null);
    }

    async getTokens(): Promise<Tokens> {
        return await this.transport.request('tokens', null);
    }

    async updateTokenSet(): Promise<void> {
        const updatedTokenSet = new TokenSet(await this.getTokens());
        this.tokenSet = updatedTokenSet;
    }

    async getState(address: Address): Promise<AccountState> {
        return await this.transport.request('account_info', [address]);
    }

    // get transaction status by its hash (e.g. 0xdead..beef)
    async getTxReceipt(txHash: string): Promise<TransactionReceipt> {
        return await this.transport.request('tx_info', [txHash]);
    }

    async getPriorityOpStatus(serialId: number): Promise<PriorityOperationReceipt> {
        return await this.transport.request('ethop_info', [serialId]);
    }

    async getConfirmationsForEthOpAmount(): Promise<number> {
        return await this.transport.request('get_confirmations_for_eth_op_amount', []);
    }

    async getEthTxForWithdrawal(withdrawal_hash: string): Promise<string> {
        return await this.transport.request('get_eth_tx_for_withdrawal', [withdrawal_hash]);
    }

    async getNFT(id: number): Promise<NFT> {
        return await this.transport.request('get_nft', [id]);
    }

    async getTokenSymbol(token: TokenLike): Promise<string> {
        if (isNFT(token)) {
            const nft = await this.getNFT(token as number);
            return nft.symbol || `NFT-${token}`;
        }
        return this.tokenSet.resolveTokenSymbol(token);
    }

    async notifyPriorityOp(serialId: number, action: 'COMMIT' | 'VERIFY'): Promise<PriorityOperationReceipt> {
        if (this.transport.subscriptionsSupported()) {
            return await new Promise((resolve) => {
                const subscribe = this.transport.subscribe(
                    'ethop_subscribe',
                    [serialId, action],
                    'ethop_unsubscribe',
                    (resp) => {
                        subscribe
                            .then((sub) => sub.unsubscribe())
                            .catch((err) => console.log(`WebSocket connection closed with reason: ${err}`));
                        resolve(resp);
                    }
                );
            });
        } else {
            while (true) {
                const priorOpStatus = await this.getPriorityOpStatus(serialId);
                const notifyDone =
                    action === 'COMMIT'
                        ? priorOpStatus.block && priorOpStatus.block.committed
                        : priorOpStatus.block && priorOpStatus.block.verified;
                if (notifyDone) {
                    return priorOpStatus;
                } else {
                    await sleep(this.pollIntervalMilliSecs);
                }
            }
        }
    }

    async notifyTransaction(hash: string, action: 'COMMIT' | 'VERIFY'): Promise<TransactionReceipt> {
        if (this.transport.subscriptionsSupported()) {
            return await new Promise((resolve) => {
                const subscribe = this.transport.subscribe('tx_subscribe', [hash, action], 'tx_unsubscribe', (resp) => {
                    subscribe
                        .then((sub) => sub.unsubscribe())
                        .catch((err) => console.log(`WebSocket connection closed with reason: ${err}`));
                    resolve(resp);
                });
            });
        } else {
            while (true) {
                const transactionStatus = await this.getTxReceipt(hash);
                const notifyDone =
                    action == 'COMMIT'
                        ? transactionStatus.block && transactionStatus.block.committed
                        : transactionStatus.block && transactionStatus.block.verified;
                if (notifyDone) {
                    return transactionStatus;
                } else {
                    await sleep(this.pollIntervalMilliSecs);
                }
            }
        }
    }

    async getTransactionFee(
        txType:
            | 'Withdraw'
            | 'Transfer'
            | 'FastWithdraw'
            | 'MintNFT'
            | 'Swap'
            | ChangePubKeyFee
            | 'WithdrawNFT'
            | 'FastWithdrawNFT'
            | LegacyChangePubKeyFee,
        address: Address,
        tokenLike: TokenLike
    ): Promise<Fee> {
        const transactionFee = await this.transport.request('get_tx_fee', [txType, address.toString(), tokenLike]);
        return {
            feeType: transactionFee.feeType,
            gasTxAmount: BigNumber.from(transactionFee.gasTxAmount),
            gasPriceWei: BigNumber.from(transactionFee.gasPriceWei),
            gasFee: BigNumber.from(transactionFee.gasFee),
            zkpFee: BigNumber.from(transactionFee.zkpFee),
            totalFee: BigNumber.from(transactionFee.totalFee)
        };
    }

    async getTransactionsBatchFee(
        txTypes: (
            | 'Withdraw'
            | 'Transfer'
            | 'FastWithdraw'
            | 'MintNFT'
            | 'WithdrawNFT'
            | 'FastWithdrawNFT'
            | ChangePubKeyFee
            | LegacyChangePubKeyFee
            | 'Swap'
        )[],
        addresses: Address[],
        tokenLike: TokenLike
    ): Promise<BigNumber> {
        const batchFee = await this.transport.request('get_txs_batch_fee_in_wei', [txTypes, addresses, tokenLike]);
        return BigNumber.from(batchFee.totalFee);
    }

    async getTokenPrice(tokenLike: TokenLike): Promise<number> {
        const tokenPrice = await this.transport.request('get_token_price', [tokenLike]);
        return parseFloat(tokenPrice);
    }

    async disconnect() {
        return await this.transport.disconnect();
    }
}

export class ETHProxy {
    private governanceContract: Governance;
    private zkSyncContract: ZkSync;
    private zksyncNFTFactory: ZkSyncNFTFactory;
    // Needed for typechain to work
    private dummySigner: ethers.VoidSigner;

    constructor(private ethersProvider: ethers.providers.Provider, public contractAddress: ContractAddress) {
        this.dummySigner = new ethers.VoidSigner(ethers.constants.AddressZero, this.ethersProvider);

        const governanceFactory = new GovernanceFactory(this.dummySigner);
        this.governanceContract = governanceFactory.attach(contractAddress.govContract);

        const zkSyncFactory = new ZkSyncFactory(this.dummySigner);
        this.zkSyncContract = zkSyncFactory.attach(contractAddress.mainContract);
    }

    getGovernanceContract(): Governance {
        return this.governanceContract;
    }

    getZkSyncContract(): ZkSync {
        return this.zkSyncContract;
    }

    // This method is very helpful for those who have already fetched the
    // default factory and want to avoid asynchorouns execution from now on
    getCachedNFTDefaultFactory(): ZkSyncNFTFactory | undefined {
        return this.zksyncNFTFactory;
    }

    async getDefaultNFTFactory(): Promise<ZkSyncNFTFactory> {
        if (this.zksyncNFTFactory) {
            return this.zksyncNFTFactory;
        }

        const nftFactoryAddress = await this.governanceContract.defaultFactory();

        const nftFactory = new ZkSyncNFTFactoryFactory(this.dummySigner);
        this.zksyncNFTFactory = nftFactory.attach(nftFactoryAddress);

        return this.zksyncNFTFactory;
    }

    async resolveTokenId(token: TokenAddress): Promise<number> {
        if (isTokenETH(token)) {
            return 0;
        } else {
            const tokenId = await this.governanceContract.tokenIds(token);
            if (tokenId == 0) {
                throw new Error(`ERC20 token ${token} is not supported`);
            }
            return tokenId;
        }
    }
}
