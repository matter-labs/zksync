import { BigNumber, BigNumberish, Contract, ContractTransaction, ethers } from 'ethers';
import { ErrorCode as EthersErrorCode } from '@ethersproject/logger';
import { EthMessageSigner } from './eth-message-signer';
import { SyncProvider } from './provider-interface';
import { BatchBuilder, BatchBuilderInternalTx } from './batch-builder';
import {
    AccountState,
    Address,
    ChangePubkeyTypes,
    NFT,
    Nonce,
    Order,
    PubKeyHash,
    SignedTransaction,
    TokenLike,
    TxEthSignature,
    TokenRatio,
    WeiRatio,
    Toggle2FARequest,
    l1ChainId
} from './types';
import {
    ERC20_APPROVE_TRESHOLD,
    ERC20_DEPOSIT_GAS_LIMIT,
    ERC20_RECOMMENDED_DEPOSIT_GAS_LIMIT,
    ETH_RECOMMENDED_DEPOSIT_GAS_LIMIT,
    getEthereumBalance,
    IERC20_INTERFACE,
    isTokenETH,
    MAX_ERC20_APPROVE_AMOUNT,
    SYNC_MAIN_CONTRACT_INTERFACE,
    getToggle2FAMessage
} from './utils';
import { Transaction, ETHOperation } from './operations';

export abstract class AbstractWallet {
    public provider: SyncProvider;

    protected constructor(public cachedAddress: Address, public accountId?: number) {}

    connect(provider: SyncProvider) {
        this.provider = provider;
        return this;
    }

    // ****************
    // Abstract getters
    //

    /**
     * Returns the current Ethereum signer connected to this wallet.
     */
    abstract ethSigner(): ethers.Signer;

    /**
     * Returns the current Ethereum **message** signer connected to this wallet.
     *
     * Ethereum message signer differs from common Ethereum signer in that message signer
     * returns Ethereum signatures along with its type (e.g. ECDSA / EIP1271).
     */
    abstract ethMessageSigner(): EthMessageSigner;

    /**
     * Returns `true` if this wallet instance has a connected L2 signer.
     */
    abstract syncSignerConnected(): boolean;

    /**
     * Returns the PubKeyHash that current *signer* uses
     * (as opposed to the one set in the account).
     */
    abstract syncSignerPubKeyHash(): Promise<PubKeyHash>;

    // *************
    // Basic getters
    //

    address(): Address {
        return this.cachedAddress;
    }

    async getCurrentPubKeyHash(): Promise<PubKeyHash> {
        return (await this.provider.getState(this.address())).committed.pubKeyHash;
    }

    async getNonce(nonce: Nonce = 'committed'): Promise<number> {
        if (nonce === 'committed') {
            return (await this.provider.getState(this.address())).committed.nonce;
        } else if (typeof nonce === 'number') {
            return nonce;
        }
    }

    async getAccountId(): Promise<number | undefined> {
        return (await this.getAccountState()).id;
    }

    async getAccountState(): Promise<AccountState> {
        return await this.provider.getState(this.address());
    }

    async resolveAccountId(): Promise<number> {
        if (this.accountId !== undefined) {
            return this.accountId;
        } else {
            const accountState = await this.getAccountState();
            if (!accountState.id) {
                throw new Error("Can't resolve account id from the zkSync node");
            }
            return accountState.id;
        }
    }

    async isCorrespondingSigningKeySet(): Promise<boolean> {
        if (!this.syncSignerConnected()) {
            throw new Error('ZKSync signer is required for current pubkey calculation.');
        }
        const currentPubKeyHash = await this.getCurrentPubKeyHash();
        const signerPubKeyHash = await this.syncSignerPubKeyHash();
        return currentPubKeyHash === signerPubKeyHash;
    }

    async isSigningKeySet(): Promise<boolean> {
        if (!this.syncSignerConnected()) {
            throw new Error('ZKSync signer is required for current pubkey calculation.');
        }
        const currentPubKeyHash = await this.getCurrentPubKeyHash();
        const zeroPubKeyHash = 'sync:0000000000000000000000000000000000000000';
        return zeroPubKeyHash !== currentPubKeyHash;
    }

    async getNFT(tokenId: number, type: 'committed' | 'verified' = 'committed'): Promise<NFT> {
        const accountState = await this.getAccountState();
        let token: NFT;
        if (type === 'committed') {
            token = accountState.committed.nfts[tokenId];
        } else {
            token = accountState.verified.nfts[tokenId];
        }
        return token;
    }

    async getBalance(token: TokenLike, type: 'committed' | 'verified' = 'committed'): Promise<BigNumber> {
        const accountState = await this.getAccountState();
        const tokenSymbol = this.provider.tokenSet.resolveTokenSymbol(token);
        let balance: BigNumberish;
        if (type === 'committed') {
            balance = accountState.committed.balances[tokenSymbol] || '0';
        } else {
            balance = accountState.verified.balances[tokenSymbol] || '0';
        }
        return BigNumber.from(balance);
    }

    async getEthereumBalance(token: TokenLike): Promise<BigNumber> {
        try {
            return await getEthereumBalance(this.ethSigner().provider, this.provider, this.cachedAddress, token);
        } catch (e) {
            this.modifyEthersError(e);
        }
    }

    // *********************
    // Batch builder methods
    //

    /**
     * Creates a batch builder instance.
     *
     * @param nonce Nonce that should be used as the nonce of the first transaction in the batch.
     * @returns Batch builder object
     */
    batchBuilder(nonce?: Nonce): BatchBuilder {
        return BatchBuilder.fromWallet(this, nonce);
    }

    /**
     * Internal method used to process transactions created via batch builder.
     * Should not be used directly.
     */
    abstract processBatchBuilderTransactions(
        startNonce: Nonce,
        txs: BatchBuilderInternalTx[]
    ): Promise<{ txs: SignedTransaction[]; signature?: TxEthSignature }>;

    // *************
    // L2 operations
    //
    // Operations below each come in three signatures:
    // - `getXXX`: get the full transaction with L2 signature.
    // - `signXXX`: get the full transaction with both L2 and L1 signatures.
    // - `XXX` or `syncXXX`: sign and send the transaction to zkSync.
    //
    // All these methods accept incomplete transaction data, and if they return signed transaction, this transaction will
    // be "completed". "Incomplete transaction data" means that e.g. account IDs are not resolved or tokens are represented
    // by their names/addresses rather than by their IDs in the zkSync network.
    //

    // Transfer part

    abstract signSyncTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction>;

    abstract syncTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: BigNumberish;
        fee?: BigNumberish;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction>;

    // ChangePubKey part

    abstract signSetSigningKey(changePubKey: {
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
        ethAuthType: ChangePubkeyTypes;
        batchHash?: string;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction>;

    abstract setSigningKey(changePubKey: {
        feeToken: TokenLike;
        ethAuthType: ChangePubkeyTypes;
        fee?: BigNumberish;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction>;

    // Withdraw part

    abstract signWithdrawFromSyncToEthereum(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction>;

    abstract withdrawFromSyncToEthereum(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: BigNumberish;
        fee?: BigNumberish;
        nonce?: Nonce;
        fastProcessing?: boolean;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction>;

    // Forced exit part

    abstract signSyncForcedExit(forcedExit: {
        target: Address;
        token: TokenLike;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction>;

    abstract syncForcedExit(forcedExit: {
        target: Address;
        token: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction>;

    // Swap part

    async signLimitOrder(order: {
        tokenSell: TokenLike;
        tokenBuy: TokenLike;
        ratio: TokenRatio | WeiRatio;
        recipient?: Address;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Order> {
        return await this.signOrder({
            ...order,
            amount: 0
        });
    }

    abstract signOrder(order: {
        tokenSell: TokenLike;
        tokenBuy: TokenLike;
        ratio: TokenRatio | WeiRatio;
        amount: BigNumberish;
        recipient?: Address;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Order>;

    abstract signSyncSwap(swap: {
        orders: [Order, Order];
        feeToken: number;
        amounts: [BigNumberish, BigNumberish];
        nonce: number;
        fee: BigNumberish;
    }): Promise<SignedTransaction>;

    abstract syncSwap(swap: {
        orders: [Order, Order];
        feeToken: TokenLike;
        amounts?: [BigNumberish, BigNumberish];
        nonce?: number;
        fee?: BigNumberish;
    }): Promise<Transaction>;

    // Mint NFT part

    abstract signMintNFT(mintNFT: {
        recipient: string;
        contentHash: string;
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
    }): Promise<SignedTransaction>;

    abstract mintNFT(mintNFT: {
        recipient: Address;
        contentHash: ethers.BytesLike;
        feeToken: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction>;

    // Withdraw NFT part

    abstract signWithdrawNFT(withdrawNFT: {
        to: string;
        token: number;
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction>;

    abstract withdrawNFT(withdrawNFT: {
        to: string;
        token: number;
        feeToken: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
        fastProcessing?: boolean;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction>;

    // Transfer NFT part

    abstract syncTransferNFT(transfer: {
        to: Address;
        token: NFT;
        feeToken: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction[]>;

    // Multi-transfer part

    // Note that in syncMultiTransfer, unlike in syncTransfer,
    // users need to specify the fee for each transaction.
    // The main reason is that multitransfer enables paying fees
    // in multiple tokens, (as long as the total sum
    // of fees is enough to cover up the fees for all of the transactions).
    // That might bring an inattentive user in a trouble like the following:
    //
    // A user wants to submit transactions in multiple tokens and
    // wants to pay the fees with only some of them. If the user forgets
    // to set the fees' value to 0 for transactions with tokens
    // he won't pay the fee with, then this user will overpay a lot.
    //
    // That's why we want the users to be explicit about fees in multitransfers.
    abstract syncMultiTransfer(
        transfers: {
            to: Address;
            token: TokenLike;
            amount: BigNumberish;
            fee: BigNumberish;
            nonce?: Nonce;
            validFrom?: number;
            validUntil?: number;
        }[]
    ): Promise<Transaction[]>;

    // Toggle 2FA part

    async getToggle2FA(enable: boolean, pubKeyHash?: PubKeyHash): Promise<Toggle2FARequest> {
        const accountId = await this.getAccountId();
        const timestamp = new Date().getTime();
        const signature = await this.ethMessageSigner().getEthMessageSignature(
            getToggle2FAMessage(enable, timestamp, pubKeyHash)
        );

        return {
            accountId,
            signature,
            timestamp,
            enable,
            pubKeyHash
        };
    }

    async toggle2FA(enable: boolean, pubKeyHash?: PubKeyHash): Promise<boolean> {
        await this.setRequiredAccountIdFromServer('Toggle 2FA');

        return await this.provider.toggle2FA(await this.getToggle2FA(enable, pubKeyHash));
    }

    // *************
    // L1 operations
    //
    // Priority operations, ones that sent through Ethereum.
    //

    async approveERC20TokenDeposits(
        token: TokenLike,
        max_erc20_approve_amount: BigNumber = MAX_ERC20_APPROVE_AMOUNT
    ): Promise<ContractTransaction> {
        if (isTokenETH(token)) {
            throw Error('ETH token does not need approval.');
        }
        const tokenAddress = this.provider.tokenSet.resolveTokenAddress(token);
        const erc20contract = new Contract(tokenAddress, IERC20_INTERFACE, this.ethSigner());

        try {
            return erc20contract.approve(this.provider.contractAddress.mainContract, max_erc20_approve_amount);
        } catch (e) {
            this.modifyEthersError(e);
        }
    }

    async depositToSyncFromEthereum(deposit: {
        depositTo: Address;
        token: TokenLike;
        amount: BigNumberish;
        ethTxOptions?: ethers.providers.TransactionRequest;
        approveDepositAmountForERC20?: boolean;
    }): Promise<ETHOperation> {
        const gasPrice = await this.ethSigner().provider.getGasPrice();

        const mainZkSyncContract = this.getZkSyncMainContract();

        let ethTransaction;

        if (isTokenETH(deposit.token)) {
            try {
                ethTransaction = await mainZkSyncContract.depositETH(deposit.depositTo, {
                    value: BigNumber.from(deposit.amount),
                    gasLimit: BigNumber.from(ETH_RECOMMENDED_DEPOSIT_GAS_LIMIT),
                    gasPrice,
                    ...deposit.ethTxOptions
                });
            } catch (e) {
                this.modifyEthersError(e);
            }
        } else {
            const tokenAddress = this.provider.tokenSet.resolveTokenAddress(deposit.token);
            // ERC20 token deposit
            const erc20contract = new Contract(tokenAddress, IERC20_INTERFACE, this.ethSigner());
            let nonce: number;
            if (deposit.approveDepositAmountForERC20) {
                try {
                    const approveTx = await erc20contract.approve(
                        this.provider.contractAddress.mainContract,
                        deposit.amount
                    );
                    nonce = approveTx.nonce + 1;
                } catch (e) {
                    this.modifyEthersError(e);
                }
            }
            const args = [
                tokenAddress,
                deposit.amount,
                deposit.depositTo,
                {
                    nonce,
                    gasPrice,
                    ...deposit.ethTxOptions
                } as ethers.providers.TransactionRequest
            ];

            // We set gas limit only if user does not set it using ethTxOptions.
            const txRequest = args[args.length - 1] as ethers.providers.TransactionRequest;
            if (txRequest.gasLimit == null) {
                try {
                    const gasEstimate = await mainZkSyncContract.estimateGas.depositERC20(...args).then(
                        (estimate) => estimate,
                        () => BigNumber.from('0')
                    );
                    const isMainnet = (await this.ethSigner().getChainId()) == 1;
                    let recommendedGasLimit =
                        isMainnet && ERC20_DEPOSIT_GAS_LIMIT[tokenAddress]
                            ? BigNumber.from(ERC20_DEPOSIT_GAS_LIMIT[tokenAddress])
                            : ERC20_RECOMMENDED_DEPOSIT_GAS_LIMIT;
                    txRequest.gasLimit = gasEstimate.gte(recommendedGasLimit) ? gasEstimate : recommendedGasLimit;
                    args[args.length - 1] = txRequest;
                } catch (e) {
                    this.modifyEthersError(e);
                }
            }

            try {
                ethTransaction = await mainZkSyncContract.depositERC20(...args);
            } catch (e) {
                this.modifyEthersError(e);
            }
        }

        return new ETHOperation(ethTransaction, this.provider);
    }

    async onchainAuthSigningKey(
        nonce: Nonce = 'committed',
        ethTxOptions?: ethers.providers.TransactionRequest
    ): Promise<ContractTransaction> {
        if (!this.syncSignerConnected()) {
            throw new Error('ZKSync signer is required for current pubkey calculation.');
        }

        const currentPubKeyHash = await this.getCurrentPubKeyHash();
        const newPubKeyHash = await this.syncSignerPubKeyHash();

        if (currentPubKeyHash === newPubKeyHash) {
            throw new Error('Current PubKeyHash is the same as new');
        }

        const numNonce = await this.getNonce(nonce);

        const mainZkSyncContract = this.getZkSyncMainContract();

        try {
            return mainZkSyncContract.setAuthPubkeyHash(newPubKeyHash.replace('sync:', '0x'), numNonce, {
                gasLimit: BigNumber.from('200000'),
                ...ethTxOptions
            });
        } catch (e) {
            this.modifyEthersError(e);
        }
    }

    async emergencyWithdraw(withdraw: {
        token: TokenLike;
        accountId?: number;
        ethTxOptions?: ethers.providers.TransactionRequest;
    }): Promise<ETHOperation> {
        const gasPrice = await this.ethSigner().provider.getGasPrice();

        let accountId: number = withdraw.accountId != null ? withdraw.accountId : await this.resolveAccountId();

        const mainZkSyncContract = this.getZkSyncMainContract();

        const tokenAddress = this.provider.tokenSet.resolveTokenAddress(withdraw.token);
        try {
            const ethTransaction = await mainZkSyncContract.requestFullExit(accountId, tokenAddress, {
                gasLimit: BigNumber.from('500000'),
                gasPrice,
                ...withdraw.ethTxOptions
            });
            return new ETHOperation(ethTransaction, this.provider);
        } catch (e) {
            this.modifyEthersError(e);
        }
    }

    async emergencyWithdrawNFT(withdrawNFT: {
        tokenId: number;
        accountId?: number;
        ethTxOptions?: ethers.providers.TransactionRequest;
    }): Promise<ETHOperation> {
        const gasPrice = await this.ethSigner().provider.getGasPrice();

        let accountId: number = withdrawNFT.accountId != null ? withdrawNFT.accountId : await this.resolveAccountId();

        const mainZkSyncContract = this.getZkSyncMainContract();

        try {
            const ethTransaction = await mainZkSyncContract.requestFullExitNFT(accountId, withdrawNFT.tokenId, {
                gasLimit: BigNumber.from('500000'),
                gasPrice,
                ...withdrawNFT.ethTxOptions
            });
            return new ETHOperation(ethTransaction, this.provider);
        } catch (e) {
            this.modifyEthersError(e);
        }
    }

    async signRegisterFactory(factoryAddress: Address): Promise<{
        signature: TxEthSignature;
        accountId: number;
        accountAddress: Address;
    }> {
        await this.setRequiredAccountIdFromServer('Sign register factory');
        const signature = await this.ethMessageSigner().ethSignRegisterFactoryMessage(
            factoryAddress,
            this.accountId,
            this.address()
        );
        return {
            signature,
            accountId: this.accountId,
            accountAddress: this.address()
        };
    }

    // **********
    // L1 getters
    //
    // Getter methods that query information from Web3.
    //

    async isOnchainAuthSigningKeySet(nonce: Nonce = 'committed'): Promise<boolean> {
        const mainZkSyncContract = this.getZkSyncMainContract();

        const numNonce = await this.getNonce(nonce);
        try {
            const onchainAuthFact = await mainZkSyncContract.authFacts(this.address(), numNonce);
            return onchainAuthFact !== '0x0000000000000000000000000000000000000000000000000000000000000000';
        } catch (e) {
            this.modifyEthersError(e);
        }
    }

    async isERC20DepositsApproved(
        token: TokenLike,
        erc20ApproveThreshold: BigNumber = ERC20_APPROVE_TRESHOLD
    ): Promise<boolean> {
        if (isTokenETH(token)) {
            throw Error('ETH token does not need approval.');
        }
        const tokenAddress = this.provider.tokenSet.resolveTokenAddress(token);
        const erc20contract = new Contract(tokenAddress, IERC20_INTERFACE, this.ethSigner());
        try {
            const currentAllowance = await erc20contract.allowance(
                this.address(),
                this.provider.contractAddress.mainContract
            );
            return BigNumber.from(currentAllowance).gte(erc20ApproveThreshold);
        } catch (e) {
            this.modifyEthersError(e);
        }
    }

    getZkSyncMainContract() {
        return new ethers.Contract(
            this.provider.contractAddress.mainContract,
            SYNC_MAIN_CONTRACT_INTERFACE,
            this.ethSigner()
        );
    }

    // ****************
    // Internal methods
    //

    protected async verifyNetworks() {
        if (this.provider.network != undefined && this.ethSigner().provider != undefined) {
            const ethNetwork = await this.ethSigner().provider.getNetwork();
            if (l1ChainId(this.provider.network) !== ethNetwork.chainId) {
                throw new Error(
                    `ETH network ${ethNetwork.name} and ZkSync network ${this.provider.network} don't match`
                );
            }
        }
    }

    protected modifyEthersError(error: any): never {
        if (this.ethSigner instanceof ethers.providers.JsonRpcSigner) {
            // List of errors that can be caused by user's actions, which have to be forwarded as-is.
            const correct_errors = [
                EthersErrorCode.NONCE_EXPIRED,
                EthersErrorCode.INSUFFICIENT_FUNDS,
                EthersErrorCode.REPLACEMENT_UNDERPRICED,
                EthersErrorCode.UNPREDICTABLE_GAS_LIMIT
            ];
            if (!correct_errors.includes(error.code)) {
                // This is an error which we don't expect
                error.message = `Ethereum smart wallet JSON RPC server returned the following error while executing an operation: "${error.message}". Please contact your smart wallet support for help.`;
            }
        }

        throw error;
    }

    protected async setRequiredAccountIdFromServer(actionName: string) {
        if (this.accountId === undefined) {
            const accountIdFromServer = await this.getAccountId();
            if (accountIdFromServer == null) {
                throw new Error(`Failed to ${actionName}: Account does not exist in the zkSync network`);
            } else {
                this.accountId = accountIdFromServer;
            }
        }
    }
}
