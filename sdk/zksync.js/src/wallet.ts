import { BigNumber, BigNumberish, Contract, ContractTransaction, ethers } from 'ethers';
import { ErrorCode } from '@ethersproject/logger';
import { ETHProxy, Provider } from './provider';
import { Signer } from './signer';
import {
    AccountState,
    Address,
    TokenLike,
    Nonce,
    PriorityOperationReceipt,
    TransactionReceipt,
    PubKeyHash,
    TxEthSignature,
    ChangePubKey,
    EthSignerType,
    SignedTransaction
} from './types';
import {
    ERC20_APPROVE_TRESHOLD,
    IERC20_INTERFACE,
    isTokenETH,
    MAX_ERC20_APPROVE_AMOUNT,
    getChangePubkeyMessage,
    SYNC_MAIN_CONTRACT_INTERFACE,
    getSignedBytesFromMessage,
    signMessagePersonalAPI,
    ERC20_DEPOSIT_GAS_LIMIT,
    getEthSignatureType
} from './utils';

const EthersErrorCode = ErrorCode;

class ZKSyncTxError extends Error {
    constructor(message: string, public value: PriorityOperationReceipt | TransactionReceipt) {
        super(message);
    }
}

export class Wallet {
    public provider: Provider;

    private constructor(
        public ethSigner: ethers.Signer,
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

        const wallet = new Wallet(ethWallet, await ethWallet.getAddress(), signer, accountId, ethSignerType);

        wallet.connect(provider);
        return wallet;
    }

    static async fromEthSignerNoKeys(
        ethWallet: ethers.Signer,
        provider: Provider,
        accountId?: number,
        ethSignerType?: EthSignerType
    ): Promise<Wallet> {
        const wallet = new Wallet(ethWallet, await ethWallet.getAddress(), undefined, accountId, ethSignerType);
        wallet.connect(provider);
        return wallet;
    }

    async getEthMessageSignature(message: string): Promise<TxEthSignature> {
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

    async signSyncTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
    }): Promise<SignedTransaction> {
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
            nonce: transfer.nonce
        };

        const stringAmount = this.provider.tokenSet.formatToken(transfer.token, transfer.amount);
        const stringFee = this.provider.tokenSet.formatToken(transfer.token, transfer.fee);
        const stringToken = this.provider.tokenSet.resolveTokenSymbol(transfer.token);
        const humanReadableTxInfo =
            `Transfer ${stringAmount} ${stringToken}\n` +
            `To: ${transfer.to.toLowerCase()}\n` +
            `Nonce: ${transfer.nonce}\n` +
            `Fee: ${stringFee} ${stringToken}\n` +
            `Account Id: ${this.accountId}`;

        const txMessageEthSignature = await this.getEthMessageSignature(humanReadableTxInfo);
        const signedTransferTransaction = this.signer.signSyncTransfer(transactionData);
        return {
            tx: signedTransferTransaction,
            ethereumSignature: txMessageEthSignature
        };
    }

    async signSyncForcedExit(forcedExit: {
        target: Address;
        token: TokenLike;
        fee: BigNumberish;
        nonce: number;
    }): Promise<SignedTransaction> {
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
            nonce: forcedExit.nonce
        };

        const signedForcedExitTransaction = this.signer.signSyncForcedExit(transactionData);

        return {
            tx: signedForcedExitTransaction
        };
    }

    async syncForcedExit(forcedExit: {
        target: Address;
        token: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
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
        }[]
    ): Promise<Transaction[]> {
        if (!this.signer) {
            throw new Error('ZKSync signer is required for sending zksync transactions.');
        }

        if (transfers.length == 0) return [];

        await this.setRequiredAccountIdFromServer('Transfer funds');

        let signedTransfers = [];

        let nextNonce = transfers[0].nonce != null ? await this.getNonce(transfers[0].nonce) : await this.getNonce();

        for (let i = 0; i < transfers.length; i++) {
            const transfer = transfers[i];
            const nonce = nextNonce;
            nextNonce += 1;

            const { tx, ethereumSignature } = await this.signSyncTransfer({
                to: transfer.to,
                token: transfer.token,
                amount: transfer.amount,
                fee: transfer.fee,
                nonce
            });

            signedTransfers.push({ tx, signature: ethereumSignature });
        }

        const transactionHashes = await this.provider.submitTxsBatch(signedTransfers);
        return transactionHashes.map((txHash, idx) => new Transaction(signedTransfers[idx], txHash, this.provider));
    }

    async syncTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: BigNumberish;
        fee?: BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction> {
        transfer.nonce = transfer.nonce != null ? await this.getNonce(transfer.nonce) : await this.getNonce();

        if (transfer.fee == null) {
            const fullFee = await this.provider.getTransactionFee('Transfer', transfer.to, transfer.token);
            transfer.fee = fullFee.totalFee;
        }
        const signedTransferTransaction = await this.signSyncTransfer(transfer as any);
        return submitSignedTransaction(signedTransferTransaction, this.provider);
    }

    async signWithdrawFromSyncToEthereum(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
    }): Promise<SignedTransaction> {
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
            nonce: withdraw.nonce
        };

        const stringAmount = this.provider.tokenSet.formatToken(withdraw.token, withdraw.amount);
        const stringFee = this.provider.tokenSet.formatToken(withdraw.token, withdraw.fee);
        const stringToken = this.provider.tokenSet.resolveTokenSymbol(withdraw.token);
        const humanReadableTxInfo =
            `Withdraw ${stringAmount} ${stringToken}\n` +
            `To: ${withdraw.ethAddress.toLowerCase()}\n` +
            `Nonce: ${withdraw.nonce}\n` +
            `Fee: ${stringFee} ${stringToken}\n` +
            `Account Id: ${this.accountId}`;

        const txMessageEthSignature = await this.getEthMessageSignature(humanReadableTxInfo);

        const signedWithdrawTransaction = this.signer.signSyncWithdraw(transactionData);

        return {
            tx: signedWithdrawTransaction,
            ethereumSignature: txMessageEthSignature
        };
    }

    async withdrawFromSyncToEthereum(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: BigNumberish;
        fee?: BigNumberish;
        nonce?: Nonce;
        fastProcessing?: boolean;
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
        const signerPubKeyHash = this.signer.pubKeyHash();
        return currentPubKeyHash === signerPubKeyHash;
    }

    async signSetSigningKey(changePubKey: {
        feeToken: TokenLike;
        fee: BigNumberish;
        nonce: number;
        onchainAuth: boolean;
    }): Promise<SignedTransaction> {
        if (!this.signer) {
            throw new Error('ZKSync signer is required for current pubkey calculation.');
        }

        const feeTokenId = await this.provider.tokenSet.resolveTokenId(changePubKey.feeToken);
        const newPubKeyHash = this.signer.pubKeyHash();

        await this.setRequiredAccountIdFromServer('Set Signing Key');

        const changePubKeyMessage = getChangePubkeyMessage(newPubKeyHash, changePubKey.nonce, this.accountId);
        const ethSignature = changePubKey.onchainAuth
            ? null
            : (await this.getEthMessageSignature(changePubKeyMessage)).signature;

        const changePubKeyTx: ChangePubKey = this.signer.signSyncChangePubKey({
            accountId: this.accountId,
            account: this.address(),
            newPkHash: this.signer.pubKeyHash(),
            nonce: changePubKey.nonce,
            feeTokenId,
            fee: BigNumber.from(changePubKey.fee).toString()
        });

        changePubKeyTx.ethSignature = ethSignature;

        return {
            tx: changePubKeyTx
        };
    }

    async setSigningKey(changePubKey: {
        feeToken: TokenLike;
        fee?: BigNumberish;
        nonce?: Nonce;
        onchainAuth?: boolean;
    }): Promise<Transaction> {
        changePubKey.nonce =
            changePubKey.nonce != null ? await this.getNonce(changePubKey.nonce) : await this.getNonce();

        if (changePubKey.onchainAuth == null) {
            changePubKey.onchainAuth = false;
        }

        if (changePubKey.fee == null) {
            changePubKey.fee = 0;

            const feeType = {
                ChangePubKey: {
                    onchainPubkeyAuth: changePubKey.onchainAuth
                }
            };
            const fullFee = await this.provider.getTransactionFee(feeType, this.address(), changePubKey.feeToken);
            changePubKey.fee = fullFee.totalFee;
        }

        const txData = await this.signSetSigningKey(changePubKey as any);

        const currentPubKeyHash = await this.getCurrentPubKeyHash();
        if (currentPubKeyHash === (txData.tx as ChangePubKey).newPkHash) {
            throw new Error('Current signing key is already set');
        }

        return submitSignedTransaction(txData, this.provider);
    }

    async isOnchainAuthSigningKeySet(nonce: Nonce = 'committed'): Promise<boolean> {
        const mainZkSyncContract = new Contract(
            this.provider.contractAddress.mainContract,
            SYNC_MAIN_CONTRACT_INTERFACE,
            this.ethSigner
        );

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
        const newPubKeyHash = this.signer.pubKeyHash();

        if (currentPubKeyHash === newPubKeyHash) {
            throw new Error('Current PubKeyHash is the same as new');
        }

        const numNonce = await this.getNonce(nonce);

        const mainZkSyncContract = new Contract(
            this.provider.contractAddress.mainContract,
            SYNC_MAIN_CONTRACT_INTERFACE,
            this.ethSigner
        );

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

    async getBalance(token: TokenLike, type: 'committed' | 'verified' = 'committed'): Promise<BigNumber> {
        const accountState = await this.getAccountState();
        const tokenSymbol = this.provider.tokenSet.resolveTokenSymbol(token);
        let balance;
        if (type === 'committed') {
            balance = accountState.committed.balances[tokenSymbol] || '0';
        } else {
            balance = accountState.verified.balances[tokenSymbol] || '0';
        }
        return BigNumber.from(balance);
    }

    async getEthereumBalance(token: TokenLike): Promise<BigNumber> {
        let balance: BigNumber;
        if (isTokenETH(token)) {
            balance = await this.ethSigner.provider.getBalance(this.cachedAddress);
        } else {
            const erc20contract = new Contract(
                this.provider.tokenSet.resolveTokenAddress(token),
                IERC20_INTERFACE,
                this.ethSigner
            );
            try {
                balance = await erc20contract.balanceOf(this.cachedAddress);
            } catch (e) {
                this.modifyEthersError(e);
            }
        }
        return balance;
    }

    async isERC20DepositsApproved(token: TokenLike): Promise<boolean> {
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
            return BigNumber.from(currentAllowance).gte(ERC20_APPROVE_TRESHOLD);
        } catch (e) {
            this.modifyEthersError(e);
        }
    }

    async approveERC20TokenDeposits(token: TokenLike): Promise<ContractTransaction> {
        if (isTokenETH(token)) {
            throw Error('ETH token does not need approval.');
        }
        const tokenAddress = this.provider.tokenSet.resolveTokenAddress(token);
        const erc20contract = new Contract(tokenAddress, IERC20_INTERFACE, this.ethSigner);

        try {
            return erc20contract.approve(this.provider.contractAddress.mainContract, MAX_ERC20_APPROVE_AMOUNT);
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

        const mainZkSyncContract = new Contract(
            this.provider.contractAddress.mainContract,
            SYNC_MAIN_CONTRACT_INTERFACE,
            this.ethSigner
        );

        let ethTransaction;

        if (isTokenETH(deposit.token)) {
            try {
                ethTransaction = await mainZkSyncContract.depositETH(deposit.depositTo, {
                    value: BigNumber.from(deposit.amount),
                    gasLimit: BigNumber.from('200000'),
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
            let nonce;
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
                        (_err) => BigNumber.from('0')
                    );
                    txRequest.gasLimit = gasEstimate.gte(ERC20_DEPOSIT_GAS_LIMIT)
                        ? gasEstimate
                        : ERC20_DEPOSIT_GAS_LIMIT;
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

    async emergencyWithdraw(withdraw: {
        token: TokenLike;
        accountId?: number;
        ethTxOptions?: ethers.providers.TransactionRequest;
    }): Promise<ETHOperation> {
        const gasPrice = await this.ethSigner.provider.getGasPrice();
        const ethProxy = new ETHProxy(this.ethSigner.provider, this.provider.contractAddress);

        let accountId;
        if (withdraw.accountId != null) {
            accountId = withdraw.accountId;
        } else if (this.accountId !== undefined) {
            accountId = this.accountId;
        } else {
            const accountState = await this.getAccountState();
            if (!accountState.id) {
                throw new Error("Can't resolve account id from the zkSync node");
            }
            accountId = accountState.id;
        }

        const mainZkSyncContract = new Contract(
            ethProxy.contractAddress.mainContract,
            SYNC_MAIN_CONTRACT_INTERFACE,
            this.ethSigner
        );

        const tokenAddress = this.provider.tokenSet.resolveTokenAddress(withdraw.token);
        try {
            const ethTransaction = await mainZkSyncContract.fullExit(accountId, tokenAddress, {
                gasLimit: BigNumber.from('500000'),
                gasPrice,
                ...withdraw.ethTxOptions
            });
            return new ETHOperation(ethTransaction, this.provider);
        } catch (e) {
            this.modifyEthersError(e);
        }
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

class ETHOperation {
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

class Transaction {
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
