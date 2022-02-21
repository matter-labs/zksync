import { BigNumber, BigNumberish, ethers } from 'ethers';
import { EthMessageSigner } from './eth-message-signer';
import { SyncProvider } from './provider-interface';
import { BatchBuilderInternalTx } from './batch-builder';
import {
    Address,
    ChangePubkeyTypes,
    EthSignerType,
    NFT,
    Nonce,
    Order,
    PubKeyHash,
    SignedTransaction,
    TokenLike,
    TxEthSignature,
    TokenRatio,
    WeiRatio
} from './types';
import { Transaction, submitSignedTransaction, submitSignedTransactionsBatch } from './operations';
import { AbstractWallet } from './abstract-wallet';

export { Transaction, ETHOperation, submitSignedTransaction, submitSignedTransactionsBatch } from './operations';

export class RemoteWallet extends AbstractWallet {
    private web3Signer: ethers.Signer;

    protected constructor(
        private web3Provider: ethers.providers.Web3Provider,
        private _ethMessageSigner: EthMessageSigner,
        cachedAddress: Address,
        accountId?: number
    ) {
        super(cachedAddress, accountId);
        this.web3Signer = web3Provider.getSigner();
    }

    // ************
    // Constructors
    //

    static async fromEthSigner(
        web3Provider: ethers.providers.Web3Provider,
        provider: SyncProvider,
        accountId?: number
    ): Promise<RemoteWallet> {
        // Since this wallet implementation requires the signer to support custom RPC method,
        // we can assume that eth signer type is a constant to avoid requesting a signature each time
        // user connects.
        const ethSignerType: EthSignerType = {
            verificationMethod: 'ERC-1271',
            isSignedMsgPrefixed: true
        };

        const ethMessageSigner = new EthMessageSigner(web3Provider.getSigner(), ethSignerType);
        const wallet = new RemoteWallet(
            web3Provider,
            ethMessageSigner,
            await web3Provider.getSigner().getAddress(),
            accountId
        );
        wallet.connect(provider);
        await wallet.verifyNetworks();
        return wallet;
    }

    // ****************
    // Abstract getters
    //

    override ethSigner(): ethers.Signer {
        return this.web3Signer;
    }

    override ethMessageSigner(): EthMessageSigner {
        return this._ethMessageSigner;
    }

    override syncSignerConnected(): boolean {
        // Sync signer is the Eth signer, which is always connected.
        return true;
    }

    override async syncSignerPubKeyHash(): Promise<PubKeyHash> {
        let pubKeyHash = await this.callExtSignerPubKeyHash();
        pubKeyHash = pubKeyHash.replace('0x', 'sync:');
        return pubKeyHash;
    }

    // *********************
    // Batch builder methods
    //

    override async processBatchBuilderTransactions(
        startNonce: Nonce,
        txs: BatchBuilderInternalTx[]
    ): Promise<{ txs: SignedTransaction[]; signature?: TxEthSignature }> {
        let nonce: number = await this.getNonce(startNonce);
        // Collect transaction bodies and set nonces in it.
        const txsToSign = txs.map((tx) => {
            tx.tx.nonce = nonce;
            nonce += 1;
            return { type: tx.type, ...tx.tx };
        });
        const signedTransactions = await this.callExtSignZkSyncBatch(txsToSign);
        // Each transaction will have its own Ethereum signature, if it's required.
        // There will be no umbrella signature for the whole batch.
        return { txs: signedTransactions };
    }

    // **************
    // L2 operations
    //

    override async signSyncTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        const signed = await this.callExtSignZkSyncBatch([{ type: 'Transfer', ...transfer }]);
        return signed[0];
    }

    override async syncTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: BigNumberish;
        fee?: BigNumberish;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction> {
        const signed = await this.signSyncTransfer(transfer as any);
        return submitSignedTransaction(signed, this.provider);
    }

    // ChangePubKey part

    override async signSetSigningKey(changePubKey: {
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
        ethAuthType: ChangePubkeyTypes;
        batchHash?: string;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        const signed = await this.callExtSignZkSyncBatch([{ type: 'ChangePubKey', ...changePubKey }]);
        return signed[0];
    }

    override async setSigningKey(changePubKey: {
        feeToken: TokenLike;
        ethAuthType: ChangePubkeyTypes;
        fee?: BigNumberish;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction> {
        const signed = await this.signSetSigningKey(changePubKey as any);
        return submitSignedTransaction(signed, this.provider);
    }

    // Withdraw part

    override async signWithdrawFromSyncToEthereum(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        const signed = await this.callExtSignZkSyncBatch([{ type: 'Withdraw', ...withdraw }]);
        return signed[0];
    }

    override async withdrawFromSyncToEthereum(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: BigNumberish;
        fee?: BigNumberish;
        nonce?: Nonce;
        fastProcessing?: boolean;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction> {
        const fastProcessing = withdraw.fastProcessing;
        const signed = await this.signWithdrawFromSyncToEthereum(withdraw as any);
        return submitSignedTransaction(signed, this.provider, fastProcessing);
    }

    // Forced exit part

    override async signSyncForcedExit(forcedExit: {
        target: Address;
        token: TokenLike;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        const signed = await this.callExtSignZkSyncBatch([{ type: 'ForcedExit', ...forcedExit }]);
        return signed[0];
    }

    override async syncForcedExit(forcedExit: {
        target: Address;
        token: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction> {
        const signed = await this.signSyncForcedExit(forcedExit as any);
        return submitSignedTransaction(signed, this.provider);
    }

    // Swap part

    override async signOrder(order: {
        tokenSell: TokenLike;
        tokenBuy: TokenLike;
        ratio: TokenRatio | WeiRatio;
        amount: BigNumberish;
        recipient?: Address;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Order> {
        return await this.callExtSignOrder({ type: 'Order', ...order });
    }

    override async signSyncSwap(swap: {
        orders: [Order, Order];
        feeToken: number;
        amounts: [BigNumberish, BigNumberish];
        nonce: number;
        fee: BigNumberish;
    }): Promise<SignedTransaction> {
        const signed = await this.callExtSignZkSyncBatch([{ type: 'Swap', ...swap }]);
        return signed[0];
    }

    override async syncSwap(swap: {
        orders: [Order, Order];
        feeToken: TokenLike;
        amounts?: [BigNumberish, BigNumberish];
        nonce?: number;
        fee?: BigNumberish;
    }): Promise<Transaction> {
        const signed = await this.signSyncSwap(swap as any);
        return submitSignedTransaction(signed, this.provider);
    }

    // Mint NFT part

    override async signMintNFT(mintNFT: {
        recipient: string;
        contentHash: string;
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
    }): Promise<SignedTransaction> {
        const signed = await this.callExtSignZkSyncBatch([{ type: 'MintNFT', ...mintNFT }]);
        return signed[0];
    }

    override async mintNFT(mintNFT: {
        recipient: Address;
        contentHash: ethers.BytesLike;
        feeToken: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction> {
        const signed = await this.signMintNFT(mintNFT as any);
        return submitSignedTransaction(signed, this.provider);
    }

    // Withdraw NFT part

    override async signWithdrawNFT(withdrawNFT: {
        to: string;
        token: number;
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        const signed = await this.callExtSignZkSyncBatch([{ type: 'WithdrawNFT', ...withdrawNFT }]);
        return signed[0];
    }

    override async withdrawNFT(withdrawNFT: {
        to: string;
        token: number;
        feeToken: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
        fastProcessing?: boolean;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction> {
        const fastProcessing = withdrawNFT.fastProcessing;
        const signed = await this.signWithdrawNFT(withdrawNFT as any);
        return submitSignedTransaction(signed, this.provider, fastProcessing);
    }

    // Transfer NFT part

    override async syncTransferNFT(transfer: {
        to: Address;
        token: NFT;
        feeToken: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction[]> {
        transfer.nonce = transfer.nonce != null ? await this.getNonce(transfer.nonce) : await this.getNonce();

        let fee: BigNumberish;
        if (transfer.fee == null) {
            fee = await this.provider.getTransactionsBatchFee(
                ['Transfer', 'Transfer'],
                [transfer.to, this.address()],
                transfer.feeToken
            );
        } else {
            fee = transfer.fee;
        }

        const txNFT = {
            to: transfer.to,
            token: transfer.token.id,
            amount: 1,
            fee: 0
        };
        const txFee = {
            to: this.address(),
            token: transfer.feeToken,
            amount: 0,
            fee
        };

        return await this.syncMultiTransfer([txNFT, txFee]);
    }

    // Multi-transfer part

    // Note: this method signature requires to specify fee in each transaction.
    // For details, see the comment on this method in `AbstractWallet` class.
    override async syncMultiTransfer(
        _transfers: {
            to: Address;
            token: TokenLike;
            amount: BigNumberish;
            fee: BigNumberish;
            nonce?: Nonce;
            validFrom?: number;
            validUntil?: number;
        }[]
    ): Promise<Transaction[]> {
        const transfers = _transfers.map((transfer) => {
            return {
                type: 'Transfer',
                ...transfer
            };
        });
        const signed = await this.callExtSignZkSyncBatch(transfers);
        return submitSignedTransactionsBatch(this.provider, signed);
    }

    // ****************
    // Internal methods
    //

    /**
     *
     * Makes all fields that represent amount to be of `string` type
     * and all fields that represent tokens to be token ids i.e. of `number` type.
     * Also, it renames `ethAddress` parameter to `to` for withdrawals.
     *
     * @param txs A list of transactions
     *
     * @returns A list of prepared transactions
     */
    protected prepareTxsBeforeSending(txs: any[]): any[] {
        const amountFields = ['amount', 'fee'];
        const tokenFields = ['token', 'feeToken', 'tokenSell', 'tokenBuy'];
        return txs.map((tx) => {
            for (const field of amountFields) {
                if (field in tx) {
                    tx[field] = BigNumber.from(tx[field]).toString();
                }
            }
            for (const field of tokenFields) {
                if (field in tx) {
                    tx[field] = this.provider.tokenSet.resolveTokenId(tx[field]);
                }
            }
            if ('amounts' in tx) {
                tx.amounts = [BigNumber.from(tx.amounts[0]).toString(), BigNumber.from(tx.amounts[1]).toString()];
            }
            if ('ethAddress' in tx) {
                tx.to = tx.ethAddress;
                delete tx.ethAddress;
            }
            return tx;
        });
    }

    /**
     * Performs an RPC call to the custom `zkSync_signBatch` method.
     * This method is specified here: https://github.com/argentlabs/argent-contracts-l2/discussions/4
     *
     * Basically, it's an addition to the WalletConnect server that accepts intentionally incomplete
     * transactions (e.g. with no account IDs resolved), and returns transactions with both L1 and L2
     * signatures.
     *
     * @param txs A list of transactions to be signed.
     *
     * @returns A list of singed transactions.
     */
    protected async callExtSignZkSyncBatch(txs: any[]): Promise<SignedTransaction[]> {
        try {
            const preparedTxs = this.prepareTxsBeforeSending(txs);
            // Response must be an array of signed transactions.
            // Transactions are flattened (ethereum signatures are on the same level as L2 signatures),
            // so we need to "unflat" each one.
            const response: any[] = await this.web3Provider.send('zkSync_signBatch', [preparedTxs]);

            const transactions = response.map((tx) => {
                const ethereumSignature = tx['ethereumSignature'];
                // Remove the L1 signature from the transaction data.
                delete tx['ethereumSignature'];
                return {
                    tx,
                    ethereumSignature
                };
            });

            return transactions;
        } catch (e) {
            console.error(`Received an error performing 'zkSync_signBatch' request: ${e.toString()}`);
            throw new Error('Wallet server returned a malformed response to the sign batch request');
        }
    }

    /**
     * Performs an RPC call to the custom `zkSync_signBatch` method.
     *
     * @param txs An order data to be signed.
     *
     * @returns The completed and signed offer.
     */
    protected async callExtSignOrder(order: any): Promise<Order> {
        try {
            const preparedOrder = this.prepareTxsBeforeSending([order]);
            // For now, we assume that the same method will be used for both signing transactions and orders.
            const signedOrder: any = (await this.web3Provider.send('zkSync_signBatch', [preparedOrder]))[0];

            // Sanity check
            if (!signedOrder['signature']) {
                throw new Error('Wallet server returned a malformed response to the sign order request');
            }

            return signedOrder as Order;
        } catch (e) {
            // TODO: Catching general error is a bad idea, as a lot of things can throw an exception.
            console.error(`Received an error performing 'zkSync_signOrder' request: ${e.toString()}`);
            throw new Error('Wallet server returned a malformed response to the sign order request');
        }
    }

    /**
     * Performs an RPC call to the custom `zkSync_signerPubKeyHash` method.
     *
     * This method should return a public key hash associated with the wallet
     */
    protected async callExtSignerPubKeyHash(): Promise<PubKeyHash> {
        try {
            const response = await this.web3Provider.send('zkSync_signerPubKeyHash', null);
            if (!response['pubKeyHash']) {
                throw new Error('Wallet server returned a malformed response to the PubKeyHash request');
            }
            return response['pubKeyHash'];
        } catch (e) {
            // TODO: Catching general error is a bad idea, as a lot of things can throw an exception.
            console.error(`Received an error performing 'zkSync_signerPubKeyHash' request: ${e.toString()}`);
            throw new Error('Wallet server returned a malformed response to the PubKeyHash request');
        }
    }
}
