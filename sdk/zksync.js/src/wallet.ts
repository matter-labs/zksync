import { BigNumber, BigNumberish, Contract, ContractTransaction, ethers } from 'ethers';
import { ErrorCode } from '@ethersproject/logger';
import { EthMessageSigner } from './eth-message-signer';
import { Provider } from './provider';
import { Create2WalletSigner, Signer } from './signer';
import { BatchBuilder } from './batch-builder';
import {
    AccountState,
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
    PriorityOperationReceipt,
    PubKeyHash,
    Ratio,
    SignedTransaction,
    Swap,
    TokenLike,
    TransactionReceipt,
    Transfer,
    TxEthSignature,
    Withdraw,
    WithdrawNFT,
    TokenRatio,
    WeiRatio
} from './types';
import {
    ERC20_APPROVE_TRESHOLD,
    ERC20_DEPOSIT_GAS_LIMIT,
    ERC20_RECOMMENDED_DEPOSIT_GAS_LIMIT,
    ETH_RECOMMENDED_DEPOSIT_GAS_LIMIT,
    getChangePubkeyLegacyMessage,
    getChangePubkeyMessage,
    getEthereumBalance,
    getSignedBytesFromMessage,
    IERC20_INTERFACE,
    isTokenETH,
    MAX_ERC20_APPROVE_AMOUNT,
    MAX_TIMESTAMP,
    signMessagePersonalAPI,
    isNFT,
    SYNC_MAIN_CONTRACT_INTERFACE
} from './utils';

const EthersErrorCode = ErrorCode;

export class ZKSyncTxError extends Error {
    constructor(message: string, public value: PriorityOperationReceipt | TransactionReceipt) {
        super(message);
    }
}

export class Wallet {
    public provider: Provider;

    private constructor(
        public ethSigner: ethers.Signer,
        public ethMessageSigner: EthMessageSigner,
        public cachedAddress: Address,
        public signer?: Signer,
        public accountId?: number,
        public ethSignerType?: EthSignerType
    ) {}

    connect(provider: Provider) {
        this.provider = provider;
        return this;
    }

    static async fromEthSigner(
        ethWallet: ethers.Signer,
        provider: Provider,
        signer?: Signer,
        accountId?: number,
        ethSignerType?: EthSignerType
    ): Promise<Wallet> {
        if (signer == null) {
            const signerResult = await Signer.fromETHSignature(ethWallet);
            signer = signerResult.signer;
            ethSignerType = ethSignerType || signerResult.ethSignatureType;
        } else if (ethSignerType == null) {
            throw new Error('If you passed signer, you must also pass ethSignerType.');
        }

        const ethMessageSigner = new EthMessageSigner(ethWallet, ethSignerType);
        const wallet = new Wallet(
            ethWallet,
            ethMessageSigner,
            await ethWallet.getAddress(),
            signer,
            accountId,
            ethSignerType
        );

        wallet.connect(provider);
        return wallet;
    }

    static async fromCreate2Data(
        syncSigner: Signer,
        provider: Provider,
        create2Data: Create2Data,
        accountId?: number
    ): Promise<Wallet> {
        const create2Signer = new Create2WalletSigner(await syncSigner.pubKeyHash(), create2Data);
        return await Wallet.fromEthSigner(create2Signer, provider, syncSigner, accountId, {
            verificationMethod: 'ERC-1271',
            isSignedMsgPrefixed: true
        });
    }

    static async fromEthSignerNoKeys(
        ethWallet: ethers.Signer,
        provider: Provider,
        accountId?: number,
        ethSignerType?: EthSignerType
    ): Promise<Wallet> {
        const ethMessageSigner = new EthMessageSigner(ethWallet, ethSignerType);
        const wallet = new Wallet(
            ethWallet,
            ethMessageSigner,
            await ethWallet.getAddress(),
            undefined,
            accountId,
            ethSignerType
        );
        wallet.connect(provider);
        return wallet;
    }

    async getEthMessageSignature(message: ethers.utils.BytesLike): Promise<TxEthSignature> {
        if (this.ethSignerType == null) {
            throw new Error('ethSignerType is unknown');
        }

        const signedBytes = getSignedBytesFromMessage(message, !this.ethSignerType.isSignedMsgPrefixed);

        const signature = await signMessagePersonalAPI(this.ethSigner, signedBytes);

        return {
            type: this.ethSignerType.verificationMethod === 'ECDSA' ? 'EthereumSignature' : 'EIP1271Signature',
            signature
        };
    }

    batchBuilder(nonce?: Nonce): BatchBuilder {
        return BatchBuilder.fromWallet(this, nonce);
    }

    async getTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Promise<Transfer> {
        if (!this.signer) {
            throw new Error('ZKSync signer is required for sending zksync transactions.');
        }

        await this.setRequiredAccountIdFromServer('Transfer funds');

        const tokenId = this.provider.tokenSet.resolveTokenId(transfer.token);

        const transactionData = {
            accountId: this.accountId,
            from: this.address(),
            to: transfer.to,
            tokenId,
            amount: transfer.amount,
            fee: transfer.fee,
            nonce: transfer.nonce,
            validFrom: transfer.validFrom,
            validUntil: transfer.validUntil
        };

        return this.signer.signSyncTransfer(transactionData);
    }

    async signSyncTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        transfer.validFrom = transfer.validFrom || 0;
        transfer.validUntil = transfer.validUntil || MAX_TIMESTAMP;
        const signedTransferTransaction = await this.getTransfer(transfer as any);

        const stringAmount = BigNumber.from(transfer.amount).isZero()
            ? null
            : this.provider.tokenSet.formatToken(transfer.token, transfer.amount);
        const stringFee = BigNumber.from(transfer.fee).isZero()
            ? null
            : this.provider.tokenSet.formatToken(transfer.token, transfer.fee);
        const stringToken = this.provider.tokenSet.resolveTokenSymbol(transfer.token);
        const ethereumSignature =
            this.ethSigner instanceof Create2WalletSigner
                ? null
                : await this.ethMessageSigner.ethSignTransfer({
                      stringAmount,
                      stringFee,
                      stringToken,
                      to: transfer.to,
                      nonce: transfer.nonce,
                      accountId: this.accountId
                  });
        return {
            tx: signedTransferTransaction,
            ethereumSignature
        };
    }

    async signRegisterFactory(factoryAddress: Address): Promise<{
        signature: TxEthSignature;
        accountId: number;
        accountAddress: Address;
    }> {
        await this.setRequiredAccountIdFromServer('Sign register factory');
        const signature = await this.ethMessageSigner.ethSignRegisterFactoryMessage(
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

    async getForcedExit(forcedExit: {
        target: Address;
        token: TokenLike;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<ForcedExit> {
        if (!this.signer) {
            throw new Error('ZKSync signer is required for sending zksync transactions.');
        }
        await this.setRequiredAccountIdFromServer('perform a Forced Exit');

        const tokenId = this.provider.tokenSet.resolveTokenId(forcedExit.token);

        const transactionData = {
            initiatorAccountId: this.accountId,
            target: forcedExit.target,
            tokenId,
            fee: forcedExit.fee,
            nonce: forcedExit.nonce,
            validFrom: forcedExit.validFrom || 0,
            validUntil: forcedExit.validUntil || MAX_TIMESTAMP
        };

        return await this.signer.signSyncForcedExit(transactionData);
    }

    async signSyncForcedExit(forcedExit: {
        target: Address;
        token: TokenLike;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        const signedForcedExitTransaction = await this.getForcedExit(forcedExit);

        const stringFee = BigNumber.from(forcedExit.fee).isZero()
            ? null
            : this.provider.tokenSet.formatToken(forcedExit.token, forcedExit.fee);
        const stringToken = this.provider.tokenSet.resolveTokenSymbol(forcedExit.token);
        const ethereumSignature =
            this.ethSigner instanceof Create2WalletSigner
                ? null
                : await this.ethMessageSigner.ethSignForcedExit({
                      stringToken,
                      stringFee,
                      target: forcedExit.target,
                      nonce: forcedExit.nonce
                  });

        return {
            tx: signedForcedExitTransaction,
            ethereumSignature
        };
    }

    async syncForcedExit(forcedExit: {
        target: Address;
        token: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction> {
        forcedExit.nonce = forcedExit.nonce != null ? await this.getNonce(forcedExit.nonce) : await this.getNonce();
        if (forcedExit.fee == null) {
            // Fee for forced exit is defined by `Withdraw` transaction type (as it's essentially just a forced withdraw).
            const fullFee = await this.provider.getTransactionFee('Withdraw', forcedExit.target, forcedExit.token);
            forcedExit.fee = fullFee.totalFee;
        }

        const signedForcedExitTransaction = await this.signSyncForcedExit(forcedExit as any);
        return submitSignedTransaction(signedForcedExitTransaction, this.provider);
    }

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
    async syncMultiTransfer(
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
        if (!this.signer) {
            throw new Error('ZKSync signer is required for sending zksync transactions.');
        }

        if (transfers.length == 0) return [];

        await this.setRequiredAccountIdFromServer('Transfer funds');

        let batch = [];
        let messages: string[] = [];

        let nextNonce = transfers[0].nonce != null ? await this.getNonce(transfers[0].nonce) : await this.getNonce();
        const batchNonce = nextNonce;

        for (let i = 0; i < transfers.length; i++) {
            const transfer = transfers[i];
            const nonce = nextNonce;
            nextNonce += 1;

            const tx: Transfer = await this.getTransfer({
                to: transfer.to,
                token: transfer.token,
                amount: transfer.amount,
                fee: transfer.fee,
                nonce,
                validFrom: transfer.validFrom || 0,
                validUntil: transfer.validUntil || MAX_TIMESTAMP
            });
            const message = await this.getTransferEthMessagePart(transfer);
            messages.push(message);
            batch.push({ tx, signature: null });
        }

        messages.push(`Nonce: ${batchNonce}`);
        const message = messages.filter((part) => part.length != 0).join('\n');
        const ethSignatures =
            this.ethSigner instanceof Create2WalletSigner
                ? []
                : [await this.ethMessageSigner.getEthMessageSignature(message)];

        const transactionHashes = await this.provider.submitTxsBatch(batch, ethSignatures);
        return transactionHashes.map((txHash, idx) => new Transaction(batch[idx], txHash, this.provider));
    }

    async syncTransferNFT(transfer: {
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

    async getLimitOrder(order: {
        tokenSell: TokenLike;
        tokenBuy: TokenLike;
        ratio: TokenRatio | WeiRatio;
        recipient?: Address;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Order> {
        return this.getOrder({
            ...order,
            amount: 0
        });
    }

    async getOrder(order: {
        tokenSell: TokenLike;
        tokenBuy: TokenLike;
        ratio: TokenRatio | WeiRatio;
        amount: BigNumberish;
        recipient?: Address;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Order> {
        if (!this.signer) {
            throw new Error('zkSync signer is required for signing an order');
        }
        await this.setRequiredAccountIdFromServer('Swap order');
        const nonce = order.nonce != null ? await this.getNonce(order.nonce) : await this.getNonce();
        const recipient = order.recipient || this.address();

        let ratio: Ratio;
        const sell = order.tokenSell;
        const buy = order.tokenBuy;

        if (!order.ratio[sell] || !order.ratio[buy]) {
            throw new Error(`Wrong tokens in the ratio object: should be ${sell} and ${buy}`);
        }

        if (order.ratio.type == 'Wei') {
            ratio = [order.ratio[sell], order.ratio[buy]];
        } else if (order.ratio.type == 'Token') {
            ratio = [
                this.provider.tokenSet.parseToken(sell, order.ratio[sell].toString()),
                this.provider.tokenSet.parseToken(buy, order.ratio[buy].toString())
            ];
        }

        const signedOrder = await this.signer.signSyncOrder({
            accountId: this.accountId,
            recipient,
            nonce,
            amount: order.amount || BigNumber.from(0),
            tokenSell: this.provider.tokenSet.resolveTokenId(order.tokenSell),
            tokenBuy: this.provider.tokenSet.resolveTokenId(order.tokenBuy),
            validFrom: order.validFrom || 0,
            validUntil: order.validUntil || MAX_TIMESTAMP,
            ratio
        });

        return this.signOrder(signedOrder);
    }

    async signOrder(order: Order): Promise<Order> {
        const stringAmount = BigNumber.from(order.amount).isZero()
            ? null
            : this.provider.tokenSet.formatToken(order.tokenSell, order.amount);
        const stringTokenSell = await this.provider.getTokenSymbol(order.tokenSell);
        const stringTokenBuy = await this.provider.getTokenSymbol(order.tokenBuy);
        const ethereumSignature =
            this.ethSigner instanceof Create2WalletSigner
                ? null
                : await this.ethMessageSigner.ethSignOrder({
                      amount: stringAmount,
                      tokenSell: stringTokenSell,
                      tokenBuy: stringTokenBuy,
                      nonce: order.nonce,
                      recipient: order.recipient,
                      ratio: order.ratio
                  });
        order.ethSignature = ethereumSignature;
        return order;
    }

    async getSwap(swap: {
        orders: [Order, Order];
        feeToken: number;
        amounts: [BigNumberish, BigNumberish];
        nonce: number;
        fee: BigNumberish;
    }): Promise<Swap> {
        if (!this.signer) {
            throw new Error('zkSync signer is required for swapping funds');
        }
        await this.setRequiredAccountIdFromServer('Swap submission');
        const feeToken = this.provider.tokenSet.resolveTokenId(swap.feeToken);

        return this.signer.signSyncSwap({
            ...swap,
            submitterId: await this.getAccountId(),
            submitterAddress: this.address(),
            feeToken
        });
    }

    async signSyncSwap(swap: {
        orders: [Order, Order];
        feeToken: number;
        amounts: [BigNumberish, BigNumberish];
        nonce: number;
        fee: BigNumberish;
    }): Promise<SignedTransaction> {
        const signedSwapTransaction = await this.getSwap(swap);
        const stringFee = BigNumber.from(swap.fee).isZero()
            ? null
            : this.provider.tokenSet.formatToken(swap.feeToken, swap.fee);
        const stringToken = this.provider.tokenSet.resolveTokenSymbol(swap.feeToken);
        const ethereumSignature =
            this.ethSigner instanceof Create2WalletSigner
                ? null
                : await this.ethMessageSigner.ethSignSwap({
                      fee: stringFee,
                      feeToken: stringToken,
                      nonce: swap.nonce
                  });

        return {
            tx: signedSwapTransaction,
            ethereumSignature: [
                ethereumSignature,
                swap.orders[0].ethSignature || null,
                swap.orders[1].ethSignature || null
            ]
        };
    }

    async syncSwap(swap: {
        orders: [Order, Order];
        feeToken: TokenLike;
        amounts?: [BigNumberish, BigNumberish];
        nonce?: number;
        fee?: BigNumberish;
    }): Promise<Transaction> {
        swap.nonce = swap.nonce != null ? await this.getNonce(swap.nonce) : await this.getNonce();
        if (swap.fee == null) {
            const fullFee = await this.provider.getTransactionFee('Swap', this.address(), swap.feeToken);
            swap.fee = fullFee.totalFee;
        }

        if (swap.amounts == null) {
            let amount0 = BigNumber.from(swap.orders[0].amount);
            let amount1 = BigNumber.from(swap.orders[1].amount);
            if (!amount0.eq(0) && !amount1.eq(0)) {
                swap.amounts = [amount0, amount1];
            } else {
                throw new Error('If amounts in orders are implicit, you must specify them during submission');
            }
        }

        const signedSwapTransaction = await this.signSyncSwap(swap as any);
        return submitSignedTransaction(signedSwapTransaction, this.provider);
    }

    async syncTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: BigNumberish;
        fee?: BigNumberish;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction> {
        transfer.nonce = transfer.nonce != null ? await this.getNonce(transfer.nonce) : await this.getNonce();

        if (transfer.fee == null) {
            const fullFee = await this.provider.getTransactionFee('Transfer', transfer.to, transfer.token);
            transfer.fee = fullFee.totalFee;
        }
        const signedTransferTransaction = await this.signSyncTransfer(transfer as any);
        return submitSignedTransaction(signedTransferTransaction, this.provider);
    }

    async getMintNFT(mintNFT: {
        recipient: string;
        contentHash: string;
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
    }): Promise<MintNFT> {
        if (!this.signer) {
            throw new Error('ZKSync signer is required for sending zksync transactions.');
        }
        await this.setRequiredAccountIdFromServer('MintNFT');

        const feeTokenId = this.provider.tokenSet.resolveTokenId(mintNFT.feeToken);
        const transactionData = {
            creatorId: this.accountId,
            creatorAddress: this.address(),
            recipient: mintNFT.recipient,
            contentHash: mintNFT.contentHash,
            feeTokenId,
            fee: mintNFT.fee,
            nonce: mintNFT.nonce
        };

        return await this.signer.signMintNFT(transactionData);
    }

    async getWithdrawNFT(withdrawNFT: {
        to: string;
        token: TokenLike;
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Promise<WithdrawNFT> {
        if (!this.signer) {
            throw new Error('ZKSync signer is required for sending zksync transactions.');
        }
        await this.setRequiredAccountIdFromServer('WithdrawNFT');

        const tokenId = this.provider.tokenSet.resolveTokenId(withdrawNFT.token);
        const feeTokenId = this.provider.tokenSet.resolveTokenId(withdrawNFT.feeToken);
        const transactionData = {
            accountId: this.accountId,
            from: this.address(),
            to: withdrawNFT.to,
            tokenId,
            feeTokenId,
            fee: withdrawNFT.fee,
            nonce: withdrawNFT.nonce,
            validFrom: withdrawNFT.validFrom,
            validUntil: withdrawNFT.validUntil
        };

        return await this.signer.signWithdrawNFT(transactionData);
    }

    async getWithdrawFromSyncToEthereum(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Promise<Withdraw> {
        if (!this.signer) {
            throw new Error('ZKSync signer is required for sending zksync transactions.');
        }
        await this.setRequiredAccountIdFromServer('Withdraw funds');

        const tokenId = this.provider.tokenSet.resolveTokenId(withdraw.token);
        const transactionData = {
            accountId: this.accountId,
            from: this.address(),
            ethAddress: withdraw.ethAddress,
            tokenId,
            amount: withdraw.amount,
            fee: withdraw.fee,
            nonce: withdraw.nonce,
            validFrom: withdraw.validFrom,
            validUntil: withdraw.validUntil
        };

        return await this.signer.signSyncWithdraw(transactionData);
    }

    async signMintNFT(mintNFT: {
        recipient: string;
        contentHash: string;
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
    }): Promise<SignedTransaction> {
        const signedMintNFTTransaction = await this.getMintNFT(mintNFT as any);

        const stringFee = BigNumber.from(mintNFT.fee).isZero()
            ? null
            : this.provider.tokenSet.formatToken(mintNFT.feeToken, mintNFT.fee);
        const stringFeeToken = this.provider.tokenSet.resolveTokenSymbol(mintNFT.feeToken);
        const ethereumSignature =
            this.ethSigner instanceof Create2WalletSigner
                ? null
                : await this.ethMessageSigner.ethSignMintNFT({
                      stringFeeToken,
                      stringFee,
                      recipient: mintNFT.recipient,
                      contentHash: mintNFT.contentHash,
                      nonce: mintNFT.nonce
                  });

        return {
            tx: signedMintNFTTransaction,
            ethereumSignature
        };
    }

    async signWithdrawNFT(withdrawNFT: {
        to: string;
        token: number;
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        withdrawNFT.validFrom = withdrawNFT.validFrom || 0;
        withdrawNFT.validUntil = withdrawNFT.validUntil || MAX_TIMESTAMP;
        const signedWithdrawNFTTransaction = await this.getWithdrawNFT(withdrawNFT as any);

        const stringFee = BigNumber.from(withdrawNFT.fee).isZero()
            ? null
            : this.provider.tokenSet.formatToken(withdrawNFT.feeToken, withdrawNFT.fee);
        const stringFeeToken = this.provider.tokenSet.resolveTokenSymbol(withdrawNFT.feeToken);
        const ethereumSignature =
            this.ethSigner instanceof Create2WalletSigner
                ? null
                : await this.ethMessageSigner.ethSignWithdrawNFT({
                      token: withdrawNFT.token,
                      to: withdrawNFT.to,
                      stringFee,
                      stringFeeToken,
                      nonce: withdrawNFT.nonce
                  });

        return {
            tx: signedWithdrawNFTTransaction,
            ethereumSignature
        };
    }

    async signWithdrawFromSyncToEthereum(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        withdraw.validFrom = withdraw.validFrom || 0;
        withdraw.validUntil = withdraw.validUntil || MAX_TIMESTAMP;
        const signedWithdrawTransaction = await this.getWithdrawFromSyncToEthereum(withdraw as any);

        const stringAmount = BigNumber.from(withdraw.amount).isZero()
            ? null
            : this.provider.tokenSet.formatToken(withdraw.token, withdraw.amount);
        const stringFee = BigNumber.from(withdraw.fee).isZero()
            ? null
            : this.provider.tokenSet.formatToken(withdraw.token, withdraw.fee);
        const stringToken = this.provider.tokenSet.resolveTokenSymbol(withdraw.token);
        const ethereumSignature =
            this.ethSigner instanceof Create2WalletSigner
                ? null
                : await this.ethMessageSigner.ethSignWithdraw({
                      stringAmount,
                      stringFee,
                      stringToken,
                      ethAddress: withdraw.ethAddress,
                      nonce: withdraw.nonce,
                      accountId: this.accountId
                  });

        return {
            tx: signedWithdrawTransaction,
            ethereumSignature
        };
    }

    async mintNFT(mintNFT: {
        recipient: Address;
        contentHash: ethers.BytesLike;
        feeToken: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction> {
        mintNFT.nonce = mintNFT.nonce != null ? await this.getNonce(mintNFT.nonce) : await this.getNonce();
        mintNFT.contentHash = ethers.utils.hexlify(mintNFT.contentHash);

        if (mintNFT.fee == null) {
            const fullFee = await this.provider.getTransactionFee('MintNFT', mintNFT.recipient, mintNFT.feeToken);
            mintNFT.fee = fullFee.totalFee;
        }

        const signedMintNFTTransaction = await this.signMintNFT(mintNFT as any);

        return submitSignedTransaction(signedMintNFTTransaction, this.provider, false);
    }

    async withdrawNFT(withdrawNFT: {
        to: string;
        token: number;
        feeToken: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
        fastProcessing?: boolean;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction> {
        withdrawNFT.nonce = withdrawNFT.nonce != null ? await this.getNonce(withdrawNFT.nonce) : await this.getNonce();
        if (!isNFT(withdrawNFT.token)) {
            throw new Error('This token ID does not correspond to an NFT');
        }

        if (withdrawNFT.fee == null) {
            const feeType = withdrawNFT.fastProcessing === true ? 'FastWithdrawNFT' : 'WithdrawNFT';

            const fullFee = await this.provider.getTransactionFee(feeType, withdrawNFT.to, withdrawNFT.feeToken);
            withdrawNFT.fee = fullFee.totalFee;
        }

        const signedWithdrawNFTTransaction = await this.signWithdrawNFT(withdrawNFT as any);

        return submitSignedTransaction(signedWithdrawNFTTransaction, this.provider, withdrawNFT.fastProcessing);
    }

    async withdrawFromSyncToEthereum(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: BigNumberish;
        fee?: BigNumberish;
        nonce?: Nonce;
        fastProcessing?: boolean;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction> {
        withdraw.nonce = withdraw.nonce != null ? await this.getNonce(withdraw.nonce) : await this.getNonce();

        if (withdraw.fee == null) {
            const feeType = withdraw.fastProcessing === true ? 'FastWithdraw' : 'Withdraw';

            const fullFee = await this.provider.getTransactionFee(feeType, withdraw.ethAddress, withdraw.token);
            withdraw.fee = fullFee.totalFee;
        }

        const signedWithdrawTransaction = await this.signWithdrawFromSyncToEthereum(withdraw as any);

        return submitSignedTransaction(signedWithdrawTransaction, this.provider, withdraw.fastProcessing);
    }

    async isSigningKeySet(): Promise<boolean> {
        if (!this.signer) {
            throw new Error('ZKSync signer is required for current pubkey calculation.');
        }
        const currentPubKeyHash = await this.getCurrentPubKeyHash();
        const signerPubKeyHash = await this.signer.pubKeyHash();
        return currentPubKeyHash === signerPubKeyHash;
    }

    async getChangePubKey(changePubKey: {
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
        ethAuthData?: ChangePubKeyOnchain | ChangePubKeyECDSA | ChangePubKeyCREATE2;
        ethSignature?: string;
        validFrom: number;
        validUntil: number;
    }): Promise<ChangePubKey> {
        if (!this.signer) {
            throw new Error('ZKSync signer is required for current pubkey calculation.');
        }

        const feeTokenId = this.provider.tokenSet.resolveTokenId(changePubKey.feeToken);
        const newPkHash = await this.signer.pubKeyHash();

        await this.setRequiredAccountIdFromServer('Set Signing Key');

        const changePubKeyTx: ChangePubKey = await this.signer.signSyncChangePubKey({
            accountId: this.accountId,
            account: this.address(),
            newPkHash,
            nonce: changePubKey.nonce,
            feeTokenId,
            fee: BigNumber.from(changePubKey.fee).toString(),
            ethAuthData: changePubKey.ethAuthData,
            ethSignature: changePubKey.ethSignature,
            validFrom: changePubKey.validFrom,
            validUntil: changePubKey.validUntil
        });

        return changePubKeyTx;
    }

    async signSetSigningKey(changePubKey: {
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
        ethAuthType: ChangePubkeyTypes;
        batchHash?: string;
        validFrom?: number;
        validUntil?: number;
    }): Promise<SignedTransaction> {
        const newPubKeyHash = await this.signer.pubKeyHash();

        let ethAuthData;
        let ethSignature;
        if (changePubKey.ethAuthType === 'Onchain') {
            ethAuthData = {
                type: 'Onchain'
            };
        } else if (changePubKey.ethAuthType === 'ECDSA') {
            await this.setRequiredAccountIdFromServer('ChangePubKey authorized by ECDSA.');
            const changePubKeyMessage = getChangePubkeyMessage(
                newPubKeyHash,
                changePubKey.nonce,
                this.accountId,
                changePubKey.batchHash
            );
            const ethSignature = (await this.getEthMessageSignature(changePubKeyMessage)).signature;
            ethAuthData = {
                type: 'ECDSA',
                ethSignature,
                batchHash: changePubKey.batchHash
            };
        } else if (changePubKey.ethAuthType === 'CREATE2') {
            if (this.ethSigner instanceof Create2WalletSigner) {
                const create2data = this.ethSigner.create2WalletData;
                ethAuthData = {
                    type: 'CREATE2',
                    creatorAddress: create2data.creatorAddress,
                    saltArg: create2data.saltArg,
                    codeHash: create2data.codeHash
                };
            } else {
                throw new Error('CREATE2 wallet authentication is only available for CREATE2 wallets');
            }
        } else if (changePubKey.ethAuthType === 'ECDSALegacyMessage') {
            await this.setRequiredAccountIdFromServer('ChangePubKey authorized by ECDSALegacyMessage.');
            const changePubKeyMessage = getChangePubkeyLegacyMessage(newPubKeyHash, changePubKey.nonce, this.accountId);
            ethSignature = (await this.getEthMessageSignature(changePubKeyMessage)).signature;
        } else {
            throw new Error('Unsupported SetSigningKey type');
        }

        const changePubkeyTxUnsigned = Object.assign(changePubKey, { ethAuthData, ethSignature });
        changePubkeyTxUnsigned.validFrom = changePubKey.validFrom || 0;
        changePubkeyTxUnsigned.validUntil = changePubKey.validUntil || MAX_TIMESTAMP;
        const changePubKeyTx = await this.getChangePubKey(changePubkeyTxUnsigned as any);

        return {
            tx: changePubKeyTx
        };
    }

    async setSigningKey(changePubKey: {
        feeToken: TokenLike;
        ethAuthType: ChangePubkeyTypes;
        fee?: BigNumberish;
        nonce?: Nonce;
        validFrom?: number;
        validUntil?: number;
    }): Promise<Transaction> {
        changePubKey.nonce =
            changePubKey.nonce != null ? await this.getNonce(changePubKey.nonce) : await this.getNonce();

        if (changePubKey.fee == null) {
            changePubKey.fee = 0;

            if (changePubKey.ethAuthType === 'ECDSALegacyMessage') {
                const feeType = {
                    ChangePubKey: {
                        onchainPubkeyAuth: true
                    }
                };
                const fullFee = await this.provider.getTransactionFee(feeType, this.address(), changePubKey.feeToken);
                changePubKey.fee = fullFee.totalFee;
            } else {
                const feeType = {
                    ChangePubKey: changePubKey.ethAuthType
                };
                const fullFee = await this.provider.getTransactionFee(feeType, this.address(), changePubKey.feeToken);
                changePubKey.fee = fullFee.totalFee;
            }
        }

        const txData = await this.signSetSigningKey(changePubKey as any);

        const currentPubKeyHash = await this.getCurrentPubKeyHash();
        if (currentPubKeyHash === (txData.tx as ChangePubKey).newPkHash) {
            throw new Error('Current signing key is already set');
        }

        return submitSignedTransaction(txData, this.provider);
    }
    getWithdrawNFTEthMessagePart(withdrawNFT: {
        to: string;
        token: number;
        feeToken: TokenLike;
        fee: BigNumberish;
    }): string {
        const stringFee = BigNumber.from(withdrawNFT.fee).isZero()
            ? null
            : this.provider.tokenSet.formatToken(withdrawNFT.feeToken, withdrawNFT.fee);
        const stringFeeToken = this.provider.tokenSet.resolveTokenSymbol(withdrawNFT.feeToken);
        return this.ethMessageSigner.getWithdrawNFTEthMessagePart({
            token: withdrawNFT.token,
            to: withdrawNFT.to,
            stringFee,
            stringFeeToken
        });
    }

    // The following methods are needed in case user decided to build
    // a message for the batch himself (e.g. in case of multi-authors batch).
    // It might seem that these belong to ethMessageSigner, however, we have
    // to resolve the token and format amount/fee before constructing the
    // transaction.
    async getTransferEthMessagePart(transfer: {
        to: Address;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
    }): Promise<string> {
        const stringAmount = BigNumber.from(transfer.amount).isZero()
            ? null
            : this.provider.tokenSet.formatToken(transfer.token, transfer.amount);
        const stringFee = BigNumber.from(transfer.fee).isZero()
            ? null
            : this.provider.tokenSet.formatToken(transfer.token, transfer.fee);
        const stringToken = await this.provider.getTokenSymbol(transfer.token);
        return this.ethMessageSigner.getTransferEthMessagePart({
            stringAmount,
            stringFee,
            stringToken,
            to: transfer.to
        });
    }

    getWithdrawEthMessagePart(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
    }): string {
        const stringAmount = BigNumber.from(withdraw.amount).isZero()
            ? null
            : this.provider.tokenSet.formatToken(withdraw.token, withdraw.amount);
        const stringFee = BigNumber.from(withdraw.fee).isZero()
            ? null
            : this.provider.tokenSet.formatToken(withdraw.token, withdraw.fee);
        const stringToken = this.provider.tokenSet.resolveTokenSymbol(withdraw.token);
        return this.ethMessageSigner.getWithdrawEthMessagePart({
            stringAmount,
            stringFee,
            stringToken,
            ethAddress: withdraw.ethAddress
        });
    }

    getChangePubKeyEthMessagePart(changePubKey: {
        pubKeyHash: string;
        feeToken: TokenLike;
        fee: BigNumberish;
    }): string {
        const stringFee = BigNumber.from(changePubKey.fee).isZero()
            ? null
            : this.provider.tokenSet.formatToken(changePubKey.feeToken, changePubKey.fee);
        const stringToken = this.provider.tokenSet.resolveTokenSymbol(changePubKey.feeToken);
        return this.ethMessageSigner.getChangePubKeyEthMessagePart({
            pubKeyHash: changePubKey.pubKeyHash,
            stringToken,
            stringFee
        });
    }

    getMintNFTEthMessagePart(mintNFT: {
        recipient: string;
        contentHash: string;
        feeToken: TokenLike;
        fee: BigNumberish;
    }): string {
        const stringFee = BigNumber.from(mintNFT.fee).isZero()
            ? null
            : this.provider.tokenSet.formatToken(mintNFT.feeToken, mintNFT.fee);
        const stringFeeToken = this.provider.tokenSet.resolveTokenSymbol(mintNFT.feeToken);
        return this.ethMessageSigner.getMintNFTEthMessagePart({
            stringFeeToken,
            stringFee,
            recipient: mintNFT.recipient,
            contentHash: mintNFT.contentHash
        });
    }

    getSwapEthSignMessagePart(swap: { fee: BigNumberish; feeToken: TokenLike }): string {
        const stringFee = BigNumber.from(swap.fee).isZero()
            ? null
            : this.provider.tokenSet.formatToken(swap.feeToken, swap.fee);
        const stringToken = this.provider.tokenSet.resolveTokenSymbol(swap.feeToken);
        return this.ethMessageSigner.getSwapEthSignMessagePart({
            fee: stringFee,
            feeToken: stringToken
        });
    }

    getForcedExitEthMessagePart(forcedExit: { target: Address; token: TokenLike; fee: BigNumberish }): string {
        const stringFee = BigNumber.from(forcedExit.fee).isZero()
            ? null
            : this.provider.tokenSet.formatToken(forcedExit.token, forcedExit.fee);
        const stringToken = this.provider.tokenSet.resolveTokenSymbol(forcedExit.token);
        return this.ethMessageSigner.getForcedExitEthMessagePart({
            stringToken,
            stringFee,
            target: forcedExit.target
        });
    }

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

    async onchainAuthSigningKey(
        nonce: Nonce = 'committed',
        ethTxOptions?: ethers.providers.TransactionRequest
    ): Promise<ContractTransaction> {
        if (!this.signer) {
            throw new Error('ZKSync signer is required for current pubkey calculation.');
        }

        const currentPubKeyHash = await this.getCurrentPubKeyHash();
        const newPubKeyHash = await this.signer.pubKeyHash();

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
        return (await this.provider.getState(this.address())).id;
    }

    address(): Address {
        return this.cachedAddress;
    }

    async getAccountState(): Promise<AccountState> {
        return this.provider.getState(this.address());
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
            return getEthereumBalance(this.ethSigner.provider, this.provider, this.cachedAddress, token);
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
        const erc20contract = new Contract(tokenAddress, IERC20_INTERFACE, this.ethSigner);
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

    async approveERC20TokenDeposits(
        token: TokenLike,
        max_erc20_approve_amount: BigNumber = MAX_ERC20_APPROVE_AMOUNT
    ): Promise<ContractTransaction> {
        if (isTokenETH(token)) {
            throw Error('ETH token does not need approval.');
        }
        const tokenAddress = this.provider.tokenSet.resolveTokenAddress(token);
        const erc20contract = new Contract(tokenAddress, IERC20_INTERFACE, this.ethSigner);

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
        const gasPrice = await this.ethSigner.provider.getGasPrice();

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
            const erc20contract = new Contract(tokenAddress, IERC20_INTERFACE, this.ethSigner);
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
                    const isMainnet = (await this.ethSigner.getChainId()) == 1;
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

    async emergencyWithdraw(withdraw: {
        token: TokenLike;
        accountId?: number;
        ethTxOptions?: ethers.providers.TransactionRequest;
    }): Promise<ETHOperation> {
        const gasPrice = await this.ethSigner.provider.getGasPrice();

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
        const gasPrice = await this.ethSigner.provider.getGasPrice();

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

    getZkSyncMainContract() {
        return new ethers.Contract(
            this.provider.contractAddress.mainContract,
            SYNC_MAIN_CONTRACT_INTERFACE,
            this.ethSigner
        );
    }

    private modifyEthersError(error: any): never {
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

    private async setRequiredAccountIdFromServer(actionName: string) {
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

export class ETHOperation {
    state: 'Sent' | 'Mined' | 'Committed' | 'Verified' | 'Failed';
    error?: ZKSyncTxError;
    priorityOpId?: BigNumber;

    constructor(public ethTx: ContractTransaction, public zkSyncProvider: Provider) {
        this.state = 'Sent';
    }

    async awaitEthereumTxCommit() {
        if (this.state !== 'Sent') return;

        const txReceipt = await this.ethTx.wait();
        for (const log of txReceipt.logs) {
            try {
                const priorityQueueLog = SYNC_MAIN_CONTRACT_INTERFACE.parseLog(log);
                if (priorityQueueLog && priorityQueueLog.args.serialId != null) {
                    this.priorityOpId = priorityQueueLog.args.serialId;
                }
            } catch {}
        }
        if (!this.priorityOpId) {
            throw new Error('Failed to parse tx logs');
        }

        this.state = 'Mined';
        return txReceipt;
    }

    async awaitReceipt(): Promise<PriorityOperationReceipt> {
        this.throwErrorIfFailedState();

        await this.awaitEthereumTxCommit();
        if (this.state !== 'Mined') return;
        const receipt = await this.zkSyncProvider.notifyPriorityOp(this.priorityOpId.toNumber(), 'COMMIT');

        if (!receipt.executed) {
            this.setErrorState(new ZKSyncTxError('Priority operation failed', receipt));
            this.throwErrorIfFailedState();
        }

        this.state = 'Committed';
        return receipt;
    }

    async awaitVerifyReceipt(): Promise<PriorityOperationReceipt> {
        await this.awaitReceipt();
        if (this.state !== 'Committed') return;

        const receipt = await this.zkSyncProvider.notifyPriorityOp(this.priorityOpId.toNumber(), 'VERIFY');

        this.state = 'Verified';

        return receipt;
    }

    private setErrorState(error: ZKSyncTxError) {
        this.state = 'Failed';
        this.error = error;
    }

    private throwErrorIfFailedState() {
        if (this.state === 'Failed') throw this.error;
    }
}

export class Transaction {
    state: 'Sent' | 'Committed' | 'Verified' | 'Failed';
    error?: ZKSyncTxError;

    constructor(public txData, public txHash: string, public sidechainProvider: Provider) {
        this.state = 'Sent';
    }

    async awaitReceipt(): Promise<TransactionReceipt> {
        this.throwErrorIfFailedState();

        if (this.state !== 'Sent') return;

        const receipt = await this.sidechainProvider.notifyTransaction(this.txHash, 'COMMIT');

        if (!receipt.success) {
            this.setErrorState(new ZKSyncTxError(`zkSync transaction failed: ${receipt.failReason}`, receipt));
            this.throwErrorIfFailedState();
        }

        this.state = 'Committed';
        return receipt;
    }

    async awaitVerifyReceipt(): Promise<TransactionReceipt> {
        await this.awaitReceipt();
        const receipt = await this.sidechainProvider.notifyTransaction(this.txHash, 'VERIFY');

        this.state = 'Verified';
        return receipt;
    }

    private setErrorState(error: ZKSyncTxError) {
        this.state = 'Failed';
        this.error = error;
    }

    private throwErrorIfFailedState() {
        if (this.state === 'Failed') throw this.error;
    }
}

export async function submitSignedTransaction(
    signedTx: SignedTransaction,
    provider: Provider,
    fastProcessing?: boolean
): Promise<Transaction> {
    const transactionHash = await provider.submitTx(signedTx.tx, signedTx.ethereumSignature, fastProcessing);
    return new Transaction(signedTx, transactionHash, provider);
}

export async function submitSignedTransactionsBatch(
    provider: Provider,
    signedTxs: SignedTransaction[],
    ethSignatures?: TxEthSignature[]
): Promise<Transaction[]> {
    const transactionHashes = await provider.submitTxsBatch(
        signedTxs.map((tx) => {
            return { tx: tx.tx, signature: tx.ethereumSignature };
        }),
        ethSignatures
    );
    return transactionHashes.map((txHash, idx) => new Transaction(signedTxs[idx], txHash, provider));
}
