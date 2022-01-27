import { BigNumber, BigNumberish, ethers } from 'ethers';
import { EthMessageSigner } from './eth-message-signer';
import { SyncProvider } from './provider-interface';
import { Create2WalletSigner, Signer, unableToSign } from './signer';
import { BatchBuilderInternalTx } from './batch-builder';
import {
    Address,
    ChangePubKey,
    ChangePubKeyCREATE2,
    ChangePubKeyECDSA,
    ChangePubKeyOnchain,
    ChangePubkeyTypes,
    Create2Data,
    EthSignerType,
    ForcedExit,
    MintNFT,
    NFT,
    Nonce,
    Order,
    PubKeyHash,
    Ratio,
    SignedTransaction,
    Swap,
    TokenLike,
    Transfer,
    TxEthSignature,
    Withdraw,
    WithdrawNFT,
    TokenRatio,
    WeiRatio
} from './types';
import { getChangePubkeyLegacyMessage, getChangePubkeyMessage, MAX_TIMESTAMP, isNFT } from './utils';
import { Transaction, submitSignedTransaction } from './operations';
import { AbstractWallet } from './abstract-wallet';

export { Transaction, ETHOperation, submitSignedTransaction, submitSignedTransactionsBatch } from './operations';

export class RemoteWallet extends AbstractWallet {
    protected constructor(
        public _ethSigner: ethers.Signer,
        public _ethMessageSigner: EthMessageSigner,
        cachedAddress: Address,
        public signer?: Signer,
        accountId?: number,
        public ethSignerType?: EthSignerType
    ) {
        super(cachedAddress, accountId);
    }

    // ************
    // Constructors
    //

    static async fromEthSigner(
        ethWallet: ethers.Signer,
        provider: SyncProvider,
        signer?: Signer,
        accountId?: number,
        ethSignerType?: EthSignerType
    ): Promise<RemoteWallet> {
        throw Error("Not implemented");
    }


    // ****************
    // Abstract getters
    //

    override ethSigner(): ethers.Signer {
        return this._ethSigner;
    }

    override ethMessageSigner(): EthMessageSigner {
        return this._ethMessageSigner;
    }

    override syncSignerConnected(): boolean {
        return this.signer !== null;
    }

    override async syncSignerPubKeyHash(): Promise<PubKeyHash> {
        return await this.signer.pubKeyHash();
    }

    // *********************
    // Batch builder methods
    //

    override async processBatchBuilderTransactions(
        startNonce: Nonce,
        txs: BatchBuilderInternalTx[]
    ): Promise<{ txs: SignedTransaction[]; signature?: TxEthSignature }> {
        throw Error("Not implemented");
    }

    // **************
    // L2 operations
    //

    override async getTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Promise<Transfer> {
        throw Error("Not implemented");
    }

    override async signSyncTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        throw Error("Not implemented");
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
        throw Error("Not implemented");
    }

    // ChangePubKey part

    override async getChangePubKey(changePubKey: {
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
        ethAuthData?: ChangePubKeyOnchain | ChangePubKeyECDSA | ChangePubKeyCREATE2;
        ethSignature?: string;
        validFrom: number;
        validUntil: number;
    }): Promise<ChangePubKey> {
        throw Error("Not implemented");
    }

    override async signSetSigningKey(changePubKey: {
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
        ethAuthType: ChangePubkeyTypes;
        batchHash?: string;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        throw Error("Not implemented");
    }

    override async setSigningKey(changePubKey: {
        feeToken: TokenLike;
        ethAuthType: ChangePubkeyTypes;
        fee?: BigNumberish;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction> {
        throw Error("Not implemented");
    }

    // Withdraw part

    override async getWithdrawFromSyncToEthereum(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Promise<Withdraw> {
        throw Error("Not implemented");
    }

    override async signWithdrawFromSyncToEthereum(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        throw Error("Not implemented");
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
        throw Error("Not implemented");
    }

    // Forced exit part

    override async getForcedExit(forcedExit: {
        target: Address;
        token: TokenLike;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<ForcedExit> {
        throw Error("Not implemented");
    }

    override async signSyncForcedExit(forcedExit: {
        target: Address;
        token: TokenLike;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        throw Error("Not implemented");
    }

    override async syncForcedExit(forcedExit: {
        target: Address;
        token: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction> {
        throw Error("Not implemented");
    }

    // Swap part

    override async getLimitOrder(order: {
        tokenSell: TokenLike;
        tokenBuy: TokenLike;
        ratio: TokenRatio | WeiRatio;
        recipient?: Address;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Order> {
        throw Error("Not implemented");
    }

    override async getOrder(order: {
        tokenSell: TokenLike;
        tokenBuy: TokenLike;
        ratio: TokenRatio | WeiRatio;
        amount: BigNumberish;
        recipient?: Address;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Order> {
        throw Error("Not implemented");
    }

    override async signOrder(order: Order): Promise<Order> {
        throw Error("Not implemented");
    }

    override async getSwap(swap: {
        orders: [Order, Order];
        feeToken: number;
        amounts: [BigNumberish, BigNumberish];
        nonce: number;
        fee: BigNumberish;
    }): Promise<Swap> {
        throw Error("Not implemented");
    }

    override async signSyncSwap(swap: {
        orders: [Order, Order];
        feeToken: number;
        amounts: [BigNumberish, BigNumberish];
        nonce: number;
        fee: BigNumberish;
    }): Promise<SignedTransaction> {
        throw Error("Not implemented");
    }

    override async syncSwap(swap: {
        orders: [Order, Order];
        feeToken: TokenLike;
        amounts?: [BigNumberish, BigNumberish];
        nonce?: number;
        fee?: BigNumberish;
    }): Promise<Transaction> {
        throw Error("Not implemented");
    }

    // Mint NFT part

    override async getMintNFT(mintNFT: {
        recipient: string;
        contentHash: string;
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
    }): Promise<MintNFT> {
        throw Error("Not implemented");
    }

    override async signMintNFT(mintNFT: {
        recipient: string;
        contentHash: string;
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
    }): Promise<SignedTransaction> {
        throw Error("Not implemented");
    }

    override async mintNFT(mintNFT: {
        recipient: Address;
        contentHash: ethers.BytesLike;
        feeToken: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction> {
        throw Error("Not implemented");
    }

    // Withdraw NFT part
    override async getWithdrawNFT(withdrawNFT: {
        to: string;
        token: TokenLike;
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Promise<WithdrawNFT> {
        throw Error("Not implemented");
    }

    override async signWithdrawNFT(withdrawNFT: {
        to: string;
        token: number;
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        throw Error("Not implemented");
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
        throw Error("Not implemented");
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
        throw Error("Not implemented");
    }

    // Multi-transfer part

    // Note: this method signature requires to specify fee in each transaction.
    // For details, see the comment on this method in `AbstractWallet` class.
    override async syncMultiTransfer(
        transfers: {
            to: Address;
            token: TokenLike;
            amount: BigNumberish;
            fee: BigNumberish;
            nonce?: Nonce;
            validFrom?: number;
            validUntil?: number;
        }[]
    ): Promise<Transaction[]> {
        throw Error("Not implemented");
    }
}
