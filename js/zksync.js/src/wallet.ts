import {Contract, ContractTransaction, ethers, utils} from "ethers";
import {ETHProxy, Provider} from "./provider";
import {Signer} from "./signer";
import {
    AccountState,
    Address,
    ChangePubKey,
    EthSignerType,
    Nonce,
    PriorityOperationReceipt,
    PubKeyHash,
    Signature,
    TokenLike,
    TransactionReceipt, TransferFrom,
    Transfer,
    TxEthSignature,
    SubscriptionTx
} from "./types";
import {
    ERC20_APPROVE_TRESHOLD,
    ERC20_DEPOSIT_GAS_LIMIT,
    getChangePubkeyMessage,
    getSignedBytesFromMessage,
    IERC20_INTERFACE,
    isTokenETH,
    MAX_ERC20_APPROVE_AMOUNT,
    signMessagePersonalAPI,
    SYNC_MAIN_CONTRACT_INTERFACE
} from "./utils";
import { privateKeyFromSeed } from "./crypto";

class ZKSyncTxError extends Error {
    constructor(
        message: string,
        public value: PriorityOperationReceipt | TransactionReceipt
    ) {
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
            throw new Error(
                "If you passed signer, you must also pass ethSignerType."
            );
        }

        const wallet = new Wallet(
            ethWallet,
            await ethWallet.getAddress(),
            signer,
            accountId,
            ethSignerType
        );

        wallet.connect(provider);
        return wallet;
    }

    static async fromEthSignerNoKeys(
        ethWallet: ethers.Signer,
        provider: Provider,
        accountId?: number,
        ethSignerType?: EthSignerType
    ): Promise<Wallet> {
        const wallet = new Wallet(
            ethWallet,
            await ethWallet.getAddress(),
            undefined,
            accountId,
            ethSignerType
        );
        wallet.connect(provider);
        return wallet;
    }

    async createDerivedWallet(postfix: string): Promise<Wallet> {
        if (!this.signer) {
            throw new Error(
                "ZKSync signer is required for sending zksync transactions."
            );
        }

        // Encode the wallet private key as a hexadecimal string, and add provided postfix.
        // Obtained string will be a seed to generate a private key for derived wallet.
        const encodedPrivateKey = utils.hexlify(this.signer.privateKey);
        const privateKeyWithPostfix = encodedPrivateKey + postfix;
        const privateKeyWithPostfixBytes = utils.toUtf8Bytes(privateKeyWithPostfix);

        const derivedWalletPrivateKey = privateKeyFromSeed(privateKeyWithPostfixBytes);

        const derivedEthWallet = new ethers.Wallet(derivedWalletPrivateKey);
        const derivedSyncWallet = await Wallet.fromEthSigner(derivedEthWallet, this.provider);

        return derivedSyncWallet;
    }

    async getEthMessageSignature(message: string): Promise<TxEthSignature> {
        if (this.ethSignerType == null) {
            throw new Error("ethSignerType is unknown");
        }

        const signedBytes = getSignedBytesFromMessage(
            message,
            !this.ethSignerType.isSignedMsgPrefixed
        );

        const signature = await signMessagePersonalAPI(
            this.ethSigner,
            signedBytes
        );

        return {
            type:
                this.ethSignerType.verificationMethod === "ECDSA"
                    ? "EthereumSignature"
                    : "EIP1271Signature",
            signature
        };
    }

    async syncMultiTransfer(transfers: {
        to: Address;
        token: TokenLike;
        amount: utils.BigNumberish;
        fee?: utils.BigNumberish;
        nonce?: Nonce;
    }[]): Promise<Transaction[]> {
        if (!this.signer) {
            throw new Error(
                "ZKSync signer is required for sending zksync transactions."
            );
        }

        if (transfers.length < 2) {
            throw new Error("Transactions batch must contain at least two transactions");
        }

        await this.setRequiredAccountIdFromServer("Transfer funds");

        let signedTransfers = [];

        let nextNonce = transfers[0].nonce != null
            ? await this.getNonce(transfers[0].nonce)
            : await this.getNonce();

        for (let i = 0; i < transfers.length; i++) {
            const transfer = transfers[i];

            const tokenId = await this.provider.tokenSet.resolveTokenId(
                transfer.token
            );
            const nonce = nextNonce;
            nextNonce += 1;

            if (transfer.fee == null) {
                const fullFee = await this.provider.getTransactionFee(
                    "Transfer",
                    transfer.to,
                    transfer.token
                );
                transfer.fee = fullFee.totalFee;
            }

            const transactionData = {
                accountId: this.accountId,
                from: this.address(),
                to: transfer.to,
                tokenId,
                amount: transfer.amount,
                fee: transfer.fee,
                nonce
            };

            const stringAmount = this.provider.tokenSet.formatToken(
                transfer.token,
                transfer.amount
            );
            const stringFee = this.provider.tokenSet.formatToken(
                transfer.token,
                transfer.fee
            );
            const stringToken = await this.provider.tokenSet.resolveTokenSymbol(
                transfer.token
            );
            const humanReadableTxInfo =
                `Transfer ${stringAmount} ${stringToken}\n` +
                `To: ${transfer.to.toLowerCase()}\n` +
                `Nonce: ${nonce}\n` +
                `Fee: ${stringFee} ${stringToken}\n` +
                `Account Id: ${this.accountId}`;

            const txMessageEthSignature = await this.getEthMessageSignature(
                humanReadableTxInfo
            );

            const signedTransferTransaction = this.signer.signSyncTransfer(
                transactionData
            );

            signedTransfers.push({tx: signedTransferTransaction, signature: txMessageEthSignature});
        }

        const transactionHashes = await this.provider.submitTxsBatch(signedTransfers);
        return transactionHashes.map(function(txHash, idx) {
                return new Transaction(
                    signedTransfers[idx],
                    txHash,
                    this.provider
                )
            }, this
        );
    }

    async createTransferNoSend(transfer: {
        to: Address;
        token: TokenLike;
        amount: utils.BigNumberish;
        fee?: utils.BigNumberish;
        nonce?: Nonce;
    }): Promise<{ transfer: Transfer, ethSignature: TxEthSignature }> {
        if (!this.signer) {
            throw new Error(
                "ZKSync signer is required for sending zksync transactions."
            );
        }

        await this.setRequiredAccountIdFromServer("Transfer funds");

        const tokenId = await this.provider.tokenSet.resolveTokenId(
            transfer.token
        );
        const nonce =
            transfer.nonce != null
                ? await this.getNonce(transfer.nonce)
                : await this.getNonce();

        if (transfer.fee == null) {
            const fullFee = await this.provider.getTransactionFee(
                "Transfer",
                transfer.to,
                transfer.token
            );
            transfer.fee = fullFee.totalFee;
        }

        const transactionData = {
            accountId: this.accountId,
            from: this.address(),
            to: transfer.to,
            tokenId,
            amount: transfer.amount,
            fee: transfer.fee,
            nonce
        };

        const stringAmount = this.provider.tokenSet.formatToken(
            transfer.token,
            transfer.amount
        );
        const stringFee = this.provider.tokenSet.formatToken(
            transfer.token,
            transfer.fee
        );
        const stringToken = await this.provider.tokenSet.resolveTokenSymbol(
            transfer.token
        );
        const humanReadableTxInfo =
            `Transfer ${stringAmount} ${stringToken}\n` +
            `To: ${transfer.to.toLowerCase()}\n` +
            `Nonce: ${nonce}\n` +
            `Fee: ${stringFee} ${stringToken}\n` +
            `Account Id: ${this.accountId}`;

        const txMessageEthSignature = await this.getEthMessageSignature(
            humanReadableTxInfo
        );

        const signedTransferTransaction = this.signer.signSyncTransfer(
            transactionData
        );

        return { transfer: signedTransferTransaction, ethSignature: txMessageEthSignature };
    }

    async syncTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: utils.BigNumberish;
        fee?: utils.BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction> {
        const signedTransfer = await this.createTransferNoSend(transfer);

        const transactionHash = await this.provider.submitTx(
            signedTransfer.transfer,
            signedTransfer.ethSignature
        );
        return new Transaction(
            signedTransfer.transfer,
            transactionHash,
            this.provider
        );
    }

    async createSubscriptionTransactions(subscriptionWallet: Wallet, months: number): Promise<SubscriptionTx[]> {
        const subscriptionTxs = [];
        const subTokenSymbol = "MLTT"; // Currently subscription token is hardcoded and it's MLTT.
        const subCost = 100; // TODO: Currently subscription cost is hardcoded and it's 100 tokens.
        const burnWalletAddress = "0x0000000000000000000000000000000000000001";

        const subPeriodSeconds = 31 * 24 * 3600; // 31 days * 24 hours * 3600 seconds.
        const oneDay = 24 * 3600; // 24 hours * 3600 seconds.
        const currentUnixTime = Math.floor(Date.now() / 1000);

        let subWalletNonce = await subscriptionWallet.getNonce();

        const tokenId = this.provider.tokenSet.resolveTokenId(subTokenSymbol);

        for (let i = 0; i < months; i++) {
            // Every tx is shifted 31 days from the previous one (first one is current time) and
            // will be valid for 24 hours.
            const validFrom = currentUnixTime + subPeriodSeconds * i;
            const validUntil = validFrom + oneDay;

            const transferFromFeeResponse = await this.provider.getTransactionFee(
                "TransferFrom",
                subscriptionWallet.address(),
                subTokenSymbol,
            );

            const transferFromFee = transferFromFeeResponse.totalFee;
            const dataToSign = {
                accountId: subscriptionWallet.accountId,
                from: this.address(),
                to: subscriptionWallet.address(),
                tokenId,
                amount: subCost,
                fee: transferFromFee,
                nonce: subWalletNonce,
                validFrom,
                validUntil,
            };
            const fromSignature = await this.signer.signSyncTransferFrom(dataToSign);

            const transferFrom = await subscriptionWallet.createTransferFromNoSend({
                from: this.address(),
                token: subTokenSymbol,
                amount: subCost,
                fee: transferFromFee,
                nonce: subWalletNonce,
                validFrom,
                validUntil,
                fromSignature,
            });

            subWalletNonce += 1;

            const burnTransfer = await subscriptionWallet.createTransferNoSend({
                to: burnWalletAddress,
                token: subTokenSymbol,
                amount: subCost,
                nonce: subWalletNonce,
            });

            subWalletNonce += 1;

            const subTx: SubscriptionTx = {
                transferToSub: transferFrom, 
                burnTx: burnTransfer.transfer,
                butnTxEthSignature: burnTransfer.ethSignature
            };
            subscriptionTxs.push(subTx);
        }

        return subscriptionTxs;
    }

    async createTransferFromNoSend(transferFrom: {
        from: Address;
        token: TokenLike;
        amount: utils.BigNumberish;
        fee?: utils.BigNumberish;
        nonce?: Nonce;
        fromSignature: Signature,
        validFrom: number,
        validUntil: number,
    }): Promise<TransferFrom> {
        if (!this.signer) {
            throw new Error(
                "ZKSync signer is required for sending zksync transactions."
            );
        }

        await this.setRequiredAccountIdFromServer("Transfer funds");

        const tokenId = await this.provider.tokenSet.resolveTokenId(
            transferFrom.token
        );
        const nonce =
            transferFrom.nonce != null
                ? await this.getNonce(transferFrom.nonce)
                : await this.getNonce();

        if (transferFrom.fee == null) {
            const fullFee = await this.provider.getTransactionFee(
                "TransferFrom",
                transferFrom.from,
                transferFrom.token
            );
            transferFrom.fee = fullFee.totalFee;
        }

        const transactionData = {
            accountId: this.accountId,
            from: transferFrom.from,
            to: this.address(),
            tokenId,
            amount: transferFrom.amount,
            fee: transferFrom.fee,
            nonce,
            validFrom: transferFrom.validFrom,
            validUntil: transferFrom.validUntil,
        };
        const toSignature = this.signer.signSyncTransferFrom(transactionData);

        const transferFromTx: TransferFrom = {
            type: "TransferFrom",
            toAccountId: this.accountId,
            from: transferFrom.from,
            to: this.address(),
            token: tokenId,
            amount: utils.bigNumberify(transferFrom.amount).toString(),
            fee: utils.bigNumberify(transferFrom.fee).toString(),
            toNonce: nonce,
            fromSignature: transferFrom.fromSignature,
            toSignature,
            validFrom: transferFrom.validFrom,
            validUntil: transferFrom.validUntil
        };

        return transferFromTx;
    }

    async syncTransferFromOtherAccount(transferFrom: {
        from: Address;
        token: TokenLike;
        amount: utils.BigNumberish;
        fee?: utils.BigNumberish;
        nonce?: Nonce;
        fromSignature: Signature,
        validFrom: number,
        validUntil: number,
    }): Promise<Transaction> {
        const transferFromTx = await this.createTransferFromNoSend(transferFrom);

        const transactionHash = await this.provider.submitTx(transferFromTx);

        return new Transaction(
            transferFromTx,
            transactionHash,
            this.provider
        );
    }

    async withdrawFromSyncToEthereum(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: utils.BigNumberish;
        fee?: utils.BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction> {
        if (!this.signer) {
            throw new Error(
                "ZKSync signer is required for sending zksync transactions."
            );
        }
        await this.setRequiredAccountIdFromServer("Withdraw funds");

        const tokenId = await this.provider.tokenSet.resolveTokenId(
            withdraw.token
        );
        const nonce =
            withdraw.nonce != null
                ? await this.getNonce(withdraw.nonce)
                : await this.getNonce();

        if (withdraw.fee == null) {
            const fullFee = await this.provider.getTransactionFee(
                "Withdraw",
                withdraw.ethAddress,
                withdraw.token
            );
            withdraw.fee = fullFee.totalFee;
        }

        const transactionData = {
            accountId: this.accountId,
            from: this.address(),
            ethAddress: withdraw.ethAddress,
            tokenId,
            amount: withdraw.amount,
            fee: withdraw.fee,
            nonce
        };

        const stringAmount = this.provider.tokenSet.formatToken(
            withdraw.token,
            withdraw.amount
        );
        const stringFee = this.provider.tokenSet.formatToken(
            withdraw.token,
            withdraw.fee
        );
        const stringToken = await this.provider.tokenSet.resolveTokenSymbol(
            withdraw.token
        );
        const humanReadableTxInfo =
            `Withdraw ${stringAmount} ${stringToken}\n` +
            `To: ${withdraw.ethAddress.toLowerCase()}\n` +
            `Nonce: ${nonce}\n` +
            `Fee: ${stringFee} ${stringToken}\n` +
            `Account Id: ${this.accountId}`;

        const txMessageEthSignature = await this.getEthMessageSignature(
            humanReadableTxInfo
        );

        const signedWithdrawTransaction = this.signer.signSyncWithdraw(
            transactionData
        );

        const submitResponse = await this.provider.submitTx(
            signedWithdrawTransaction,
            txMessageEthSignature
        );
        return new Transaction(
            signedWithdrawTransaction,
            submitResponse,
            this.provider
        );
    }

    async isSigningKeySet(): Promise<boolean> {
        if (!this.signer) {
            throw new Error(
                "ZKSync signer is required for current pubkey calculation."
            );
        }
        const currentPubKeyHash = await this.getCurrentPubKeyHash();
        const signerPubKeyHash = this.signer.pubKeyHash();
        return currentPubKeyHash === signerPubKeyHash;
    }

    async setSigningKey(
        nonce: Nonce = "committed",
        onchainAuth = false
    ): Promise<Transaction> {
        if (!this.signer) {
            throw new Error(
                "ZKSync signer is required for current pubkey calculation."
            );
        }

        const currentPubKeyHash = await this.getCurrentPubKeyHash();
        const newPubKeyHash = this.signer.pubKeyHash();

        if (currentPubKeyHash === newPubKeyHash) {
            throw new Error("Current signing key is already set");
        }

        await this.setRequiredAccountIdFromServer("Set Signing Key");

        const numNonce = await this.getNonce(nonce);

        const changePubKeyMessage = getChangePubkeyMessage(
            newPubKeyHash,
            numNonce,
            this.accountId
        );
        const ethSignature = onchainAuth
            ? null
            : (await this.getEthMessageSignature(changePubKeyMessage))
                  .signature;

        const txData: ChangePubKey = {
            type: "ChangePubKey",
            accountId: this.accountId,
            account: this.address(),
            newPkHash: this.signer.pubKeyHash(),
            nonce: numNonce,
            ethSignature
        };

        const transactionHash = await this.provider.submitTx(txData);
        return new Transaction(txData, transactionHash, this.provider);
    }

    async isOnchainAuthSigningKeySet(
        nonce: Nonce = "committed"
    ): Promise<boolean> {
        const mainZkSyncContract = new Contract(
            this.provider.contractAddress.mainContract,
            SYNC_MAIN_CONTRACT_INTERFACE,
            this.ethSigner
        );

        const numNonce = await this.getNonce(nonce);
        const onchainAuthFact = await mainZkSyncContract.authFacts(
            this.address(),
            numNonce
        );
        return (
            onchainAuthFact !==
            "0x0000000000000000000000000000000000000000000000000000000000000000"
        );
    }

    async onchainAuthSigningKey(
        nonce: Nonce = "committed",
        ethTxOptions?: ethers.providers.TransactionRequest
    ): Promise<ContractTransaction> {
        if (!this.signer) {
            throw new Error(
                "ZKSync signer is required for current pubkey calculation."
            );
        }

        const currentPubKeyHash = await this.getCurrentPubKeyHash();
        const newPubKeyHash = this.signer.pubKeyHash();

        if (currentPubKeyHash == newPubKeyHash) {
            throw new Error("Current PubKeyHash is the same as new");
        }

        const numNonce = await this.getNonce(nonce);

        const mainZkSyncContract = new Contract(
            this.provider.contractAddress.mainContract,
            SYNC_MAIN_CONTRACT_INTERFACE,
            this.ethSigner
        );

        const ethTransaction = await mainZkSyncContract.setAuthPubkeyHash(
            newPubKeyHash.replace("sync:", "0x"),
            numNonce,
            {
                gasLimit: utils.bigNumberify("200000"),
                ...ethTxOptions
            }
        );

        return ethTransaction;
    }

    async getCurrentPubKeyHash(): Promise<PubKeyHash> {
        return (await this.provider.getState(this.address())).committed
            .pubKeyHash;
    }

    async getNonce(nonce: Nonce = "committed"): Promise<number> {
        if (nonce == "committed") {
            return (await this.provider.getState(this.address())).committed
                .nonce;
        } else if (typeof nonce == "number") {
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

    async getBalance(
        token: TokenLike,
        type: "committed" | "verified" = "committed"
    ): Promise<utils.BigNumber> {
        const accountState = await this.getAccountState();
        const tokenSymbol = this.provider.tokenSet.resolveTokenSymbol(token);
        let balance;
        if (type === "committed") {
            balance = accountState.committed.balances[tokenSymbol] || "0";
        } else {
            balance = accountState.verified.balances[tokenSymbol] || "0";
        }
        return utils.bigNumberify(balance);
    }

    async getEthereumBalance(token: TokenLike): Promise<utils.BigNumber> {
        let balance: utils.BigNumber;
        if (isTokenETH(token)) {
            balance = await this.ethSigner.provider.getBalance(
                this.cachedAddress
            );
        } else {
            const erc20contract = new Contract(
                this.provider.tokenSet.resolveTokenAddress(token),
                IERC20_INTERFACE,
                this.ethSigner
            );
            balance = await erc20contract.balanceOf(this.cachedAddress);
        }
        return balance;
    }

    async isERC20DepositsApproved(token: TokenLike): Promise<boolean> {
        if (isTokenETH(token)) {
            throw Error("ETH token does not need approval.");
        }
        const tokenAddress = this.provider.tokenSet.resolveTokenAddress(token);
        const erc20contract = new Contract(
            tokenAddress,
            IERC20_INTERFACE,
            this.ethSigner
        );
        const currentAllowance = await erc20contract.allowance(
            this.address(),
            this.provider.contractAddress.mainContract
        );
        return utils.bigNumberify(currentAllowance).gte(ERC20_APPROVE_TRESHOLD);
    }

    async approveERC20TokenDeposits(
        token: TokenLike
    ): Promise<ContractTransaction> {
        if (isTokenETH(token)) {
            throw Error("ETH token does not need approval.");
        }
        const tokenAddress = this.provider.tokenSet.resolveTokenAddress(token);
        const erc20contract = new Contract(
            tokenAddress,
            IERC20_INTERFACE,
            this.ethSigner
        );

        return erc20contract.approve(
            this.provider.contractAddress.mainContract,
            MAX_ERC20_APPROVE_AMOUNT
        );
    }

    async depositToSyncFromEthereum(deposit: {
        depositTo: Address;
        token: TokenLike;
        amount: utils.BigNumberish;
        ethTxOptions?: ethers.providers.TransactionRequest;
        approveDepositAmountForERC20?: boolean;
    }): Promise<ETHOperation> {
        const gasPrice = await this.ethSigner.provider.getGasPrice();

        const ethProxy = new ETHProxy(
            this.ethSigner.provider,
            this.provider.contractAddress
        );

        const mainZkSyncContract = new Contract(
            this.provider.contractAddress.mainContract,
            SYNC_MAIN_CONTRACT_INTERFACE,
            this.ethSigner
        );

        let ethTransaction;

        if (isTokenETH(deposit.token)) {
            ethTransaction = await mainZkSyncContract.depositETH(
                deposit.depositTo,
                {
                    value: utils.bigNumberify(deposit.amount),
                    gasLimit: utils.bigNumberify("200000"),
                    gasPrice,
                    ...deposit.ethTxOptions
                }
            );
        } else {
            const tokenAddress = this.provider.tokenSet.resolveTokenAddress(
                deposit.token
            );
            // ERC20 token deposit
            const erc20contract = new Contract(
                tokenAddress,
                IERC20_INTERFACE,
                this.ethSigner
            );
            let nonce;
            if (deposit.approveDepositAmountForERC20) {
                const approveTx = await erc20contract.approve(
                    this.provider.contractAddress.mainContract,
                    deposit.amount
                );
                nonce = approveTx.nonce + 1;
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
            const txRequest = args[
                args.length - 1
            ] as ethers.providers.TransactionRequest;
            if (txRequest.gasLimit == null) {
                const gasEstimate = await mainZkSyncContract.estimate
                    .depositERC20(...args)
                    .then(
                        estimate => estimate,
                        _err => utils.bigNumberify("0")
                    );
                txRequest.gasLimit = gasEstimate.gte(ERC20_DEPOSIT_GAS_LIMIT)
                    ? gasEstimate
                    : ERC20_DEPOSIT_GAS_LIMIT;
                args[args.length - 1] = txRequest;
            }

            ethTransaction = await mainZkSyncContract.depositERC20(...args);
        }

        return new ETHOperation(ethTransaction, this.provider);
    }

    async emergencyWithdraw(withdraw: {
        token: TokenLike;
        accountId?: number;
        ethTxOptions?: ethers.providers.TransactionRequest;
    }): Promise<ETHOperation> {
        const gasPrice = await this.ethSigner.provider.getGasPrice();
        const ethProxy = new ETHProxy(
            this.ethSigner.provider,
            this.provider.contractAddress
        );

        let accountId;
        if (withdraw.accountId != null) {
            accountId = withdraw.accountId;
        } else if (this.accountId !== undefined) {
            accountId = this.accountId;
        } else {
            const accountState = await this.getAccountState();
            if (!accountState.id) {
                throw new Error(
                    "Can't resolve account id from the zkSync node"
                );
            }
            accountId = accountState.id;
        }

        const mainZkSyncContract = new Contract(
            ethProxy.contractAddress.mainContract,
            SYNC_MAIN_CONTRACT_INTERFACE,
            this.ethSigner
        );

        const tokenAddress = this.provider.tokenSet.resolveTokenAddress(
            withdraw.token
        );
        const ethTransaction = await mainZkSyncContract.fullExit(
            accountId,
            tokenAddress,
            {
                gasLimit: utils.bigNumberify("500000"),
                gasPrice,
                ...withdraw.ethTxOptions
            }
        );

        return new ETHOperation(ethTransaction, this.provider);
    }

    private async setRequiredAccountIdFromServer(actionName: string) {
        if (this.accountId === undefined) {
            const accountIdFromServer = await this.getAccountId();
            if (accountIdFromServer == null) {
                throw new Error(
                    `Failed to ${actionName}: Account does not exist in the zkSync network`
                );
            } else {
                this.accountId = accountIdFromServer;
            }
        }
    }
}

class ETHOperation {
    state: "Sent" | "Mined" | "Committed" | "Verified" | "Failed";
    error?: ZKSyncTxError;
    priorityOpId?: utils.BigNumber;

    constructor(
        public ethTx: ContractTransaction,
        public zkSyncProvider: Provider
    ) {
        this.state = "Sent";
    }

    async awaitEthereumTxCommit() {
        if (this.state != "Sent") return;

        const txReceipt = await this.ethTx.wait();
        for (const log of txReceipt.logs) {
            const priorityQueueLog = SYNC_MAIN_CONTRACT_INTERFACE.parseLog(log);
            if (priorityQueueLog && priorityQueueLog.values.serialId != null) {
                this.priorityOpId = priorityQueueLog.values.serialId;
            }
        }
        if (!this.priorityOpId) {
            throw new Error("Failed to parse tx logs");
        }

        this.state = "Mined";
        return txReceipt;
    }

    async awaitReceipt(): Promise<PriorityOperationReceipt> {
        this.throwErrorIfFailedState();

        await this.awaitEthereumTxCommit();
        if (this.state != "Mined") return;
        const receipt = await this.zkSyncProvider.notifyPriorityOp(
            this.priorityOpId.toNumber(),
            "COMMIT"
        );

        if (!receipt.executed) {
            this.setErrorState(
                new ZKSyncTxError("Priority operation failed", receipt)
            );
            this.throwErrorIfFailedState();
        }

        this.state = "Committed";
        return receipt;
    }

    async awaitVerifyReceipt(): Promise<PriorityOperationReceipt> {
        await this.awaitReceipt();
        if (this.state != "Committed") return;

        const receipt = await this.zkSyncProvider.notifyPriorityOp(
            this.priorityOpId.toNumber(),
            "VERIFY"
        );

        this.state = "Verified";

        return receipt;
    }

    private setErrorState(error: ZKSyncTxError) {
        this.state = "Failed";
        this.error = error;
    }

    private throwErrorIfFailedState() {
        if (this.state == "Failed") throw this.error;
    }
}

class Transaction {
    state: "Sent" | "Committed" | "Verified" | "Failed";
    error?: ZKSyncTxError;

    constructor(
        public txData,
        public txHash: string,
        public sidechainProvider: Provider
    ) {
        this.state = "Sent";
    }

    async awaitReceipt(): Promise<TransactionReceipt> {
        this.throwErrorIfFailedState();

        if (this.state !== "Sent") return;

        const receipt = await this.sidechainProvider.notifyTransaction(
            this.txHash,
            "COMMIT"
        );

        if (!receipt.success) {
            this.setErrorState(
                new ZKSyncTxError(
                    `zkSync transaction failed: ${receipt.failReason}`,
                    receipt
                )
            );
            this.throwErrorIfFailedState();
        }

        this.state = "Committed";
        return receipt;
    }

    async awaitVerifyReceipt(): Promise<TransactionReceipt> {
        await this.awaitReceipt();
        const receipt = await this.sidechainProvider.notifyTransaction(
            this.txHash,
            "VERIFY"
        );

        this.state = "Verified";
        return receipt;
    }

    private setErrorState(error: ZKSyncTxError) {
        this.state = "Failed";
        this.error = error;
    }

    private throwErrorIfFailedState() {
        if (this.state == "Failed") throw this.error;
    }
}
