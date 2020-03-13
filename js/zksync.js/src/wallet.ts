import { Contract, ContractTransaction, ethers, utils } from "ethers";
import { ETHProxy, Provider } from "./provider";
import { serializeAddress, serializeNonce, Signer } from "./signer";
import {
    AccountState,
    Address,
    TokenLike,
    Nonce,
    PriorityOperationReceipt,
    TransactionReceipt,
    PubKeyHash
} from "./types";
import {
    IERC20_INTERFACE,
    isTokenETH, signChangePubkeyMessage,
    SYNC_MAIN_CONTRACT_INTERFACE,
} from "./utils";

export class Wallet {
    public provider: Provider;

    private constructor(
        public ethSigner: ethers.Signer,
        public cachedAddress: Address,
        public signer?: Signer,
    ) {}

    connect(provider: Provider) {
        this.provider = provider;
        return this;
    }

    static async fromEthSigner(
        ethWallet: ethers.Signer,
        provider: Provider,
        signer?: Signer
    ): Promise<Wallet> {
        const walletSigner = signer ? signer : await Signer.fromETHSignature(ethWallet);
        const wallet = new Wallet(
            ethWallet,
            await ethWallet.getAddress(),
            walletSigner,
    );
        wallet.connect(provider);
        return wallet;
    }

    static async fromEthSignerNoKeys(
        ethWallet: ethers.Signer,
        provider: Provider,
    ): Promise<Wallet> {
        const wallet = new Wallet(
            ethWallet,
            await ethWallet.getAddress()
        );
        wallet.connect(provider);
        return wallet;
    }

    async syncTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction> {
        if (!this.signer) {
            throw new Error("ZKSync signer is required for sending zksync transactions.");
        }

        const tokenId = await this.provider.tokenSet.resolveTokenId(
            transfer.token
        );
        const nonce =
            transfer.nonce != null
                ? await this.getNonce(transfer.nonce)
                : await this.getNonce();
        const transactionData = {
            from: this.address(),
            to: transfer.to,
            tokenId,
            amount: transfer.amount,
            fee: transfer.fee,
            nonce
        };

        const stringAmount = utils.formatEther(transfer.amount);
        const stringFee = utils.formatEther(transfer.fee);
        const stringToken = await this.provider.tokenSet.resolveTokenSymbol(
            transfer.token
        );
        const humanReadableTxInfo = `Transfer ${stringAmount} ${stringToken}\n`
            + `To: ${transfer.to.toLowerCase()}\n`
            + `Nonce: ${nonce}\n`
            + `Fee: ${stringFee} ${stringToken}`;

        const txMessageEthSignature = await this.ethSigner.signMessage(humanReadableTxInfo);

        const signedTransferTransaction = this.signer.signSyncTransfer(
            transactionData
        );

        const transactionHash = await this.provider.submitTx(
            signedTransferTransaction,
            txMessageEthSignature,
        );
        return new Transaction(
            signedTransferTransaction,
            transactionHash,
            this.provider
        );
    }

    async withdrawFromSyncToEthereum(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction> {
        if (!this.signer) {
            throw new Error("ZKSync signer is required for sending zksync transactions.");
        }

        const tokenId = await this.provider.tokenSet.resolveTokenId(
            withdraw.token
        );
        const nonce =
            withdraw.nonce != null
                ? await this.getNonce(withdraw.nonce)
                : await this.getNonce();
        const transactionData = {
            from: this.address(),
            ethAddress: withdraw.ethAddress,
            tokenId,
            amount: withdraw.amount,
            fee: withdraw.fee,
            nonce
        };

        const stringAmount = utils.formatEther(withdraw.amount);
        const stringFee = utils.formatEther(withdraw.fee);
        const stringToken = await this.provider.tokenSet.resolveTokenSymbol(
            withdraw.token
        );
        const humanReadableTxInfo = `Withdraw ${stringAmount} ${stringToken}\n`
            + `To: ${withdraw.ethAddress.toLowerCase()}\n`
            + `Nonce: ${nonce}\n`
            + `Fee: ${stringFee} ${stringToken}`;
        
        const txMessageEthSignature = await this.ethSigner.signMessage(humanReadableTxInfo);

        const signedWithdrawTransaction = this.signer.signSyncWithdraw(
            transactionData
        );

        const submitResponse = await this.provider.submitTx(
            signedWithdrawTransaction,
            txMessageEthSignature,
        );
        return new Transaction(
            signedWithdrawTransaction,
            submitResponse,
            this.provider
        );
    }

    async isSigningKeySet(): Promise<boolean> {
        if (!this.signer) {
            throw new Error("ZKSync signer is required for current pubkey calculation.");
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
            throw new Error("ZKSync signer is required for current pubkey calculation.");
        }

        const currentPubKeyHash = await this.getCurrentPubKeyHash();
        const newPubKeyHash = this.signer.pubKeyHash();

        if (currentPubKeyHash == newPubKeyHash) {
            throw new Error("Current signing key is set already");
        }

        const numNonce = await this.getNonce(nonce);
        const ethSignature = onchainAuth
            ? null
            : await signChangePubkeyMessage(this.ethSigner, newPubKeyHash, numNonce);

        const txData = {
            type: "ChangePubKey",
            account: this.address(),
            newPkHash: this.signer.pubKeyHash(),
            nonce: numNonce,
            ethSignature
        };

        const transactionHash = await this.provider.submitTx(txData);
        return new Transaction(txData, transactionHash, this.provider);
    }

    async onchainAuthSigningKey(
        nonce: Nonce = "committed",
        ethTxOptions?: ethers.providers.TransactionRequest,
    ): Promise<ContractTransaction> {
        if (!this.signer) {
            throw new Error("ZKSync signer is required for current pubkey calculation.");
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

        const ethTransaction = await mainZkSyncContract.authPubkeyHash(
            newPubKeyHash.replace("sync:", "0x"),
            numNonce,
            {
                gasLimit: utils.bigNumberify("200000"),
                ...ethTxOptions,
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

    async depositToSyncFromEthereum(deposit: {
        depositTo: Address;
        token: TokenLike;
        amount: utils.BigNumberish;
        maxFeeInETHToken?: utils.BigNumberish;
        ethTxOptions?: ethers.providers.TransactionRequest;
    }): Promise<ETHOperation> {
        const gasPrice = await this.ethSigner.provider.getGasPrice();

        const ethProxy = new ETHProxy(
            this.ethSigner.provider,
            this.provider.contractAddress
        );

        let maxFeeInETHToken;
        if (deposit.maxFeeInETHToken != null) {
            maxFeeInETHToken = deposit.maxFeeInETHToken;
        } else {
            maxFeeInETHToken = await ethProxy.estimateDepositFeeInETHToken(
                deposit.token,
                gasPrice
            );
        }
        const mainZkSyncContract = new Contract(
            this.provider.contractAddress.mainContract,
            SYNC_MAIN_CONTRACT_INTERFACE,
            this.ethSigner
        );

        let ethTransaction;

        if (isTokenETH(deposit.token)) {
            ethTransaction = await mainZkSyncContract.depositETH(
                deposit.amount,
                deposit.depositTo,
                {
                    value: utils
                        .bigNumberify(deposit.amount)
                        .add(maxFeeInETHToken),
                    gasLimit: utils.bigNumberify("200000"),
                    gasPrice,
                    ...deposit.ethTxOptions,
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
            const approveTx = await erc20contract.approve(
                this.provider.contractAddress.mainContract,
                deposit.amount,
                {
                    ...deposit.ethTxOptions,
                }
            );
            ethTransaction = await mainZkSyncContract.depositERC20(
                tokenAddress,
                deposit.amount,
                deposit.depositTo,
                {
                    gasLimit: utils.bigNumberify("250000"),
                    value: maxFeeInETHToken,
                    nonce: approveTx.nonce + 1,
                    gasPrice
                }
            );
        }

        return new ETHOperation(ethTransaction, this.provider);
    }

    async emergencyWithdraw(withdraw: {
        token: TokenLike;
        maxFeeInETHToken?: utils.BigNumberish;
        accountId?: number;
        ethTxOptions?: ethers.providers.TransactionRequest;
    }): Promise<ETHOperation> {
        const gasPrice = await this.ethSigner.provider.getGasPrice();
        const ethProxy = new ETHProxy(
            this.ethSigner.provider,
            this.provider.contractAddress
        );

        let maxFeeInETHToken;
        if (withdraw.maxFeeInETHToken != null) {
            maxFeeInETHToken = withdraw.maxFeeInETHToken;
        } else {
            maxFeeInETHToken = await ethProxy.estimateEmergencyWithdrawFeeInETHToken(
                gasPrice
            );
        }

        let accountId;
        if (withdraw.accountId != null) {
            accountId = withdraw.accountId;
        } else {
            const accountState = await this.getAccountState();
            if (!accountState.id) {
                throw new Error(
                    "Can't resolve account id from the ZK Sync node"
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
                value: maxFeeInETHToken,
                gasPrice,
                ...withdraw.ethTxOptions,
            }
        );

        return new ETHOperation(ethTransaction, this.provider);
    }
}

class ETHOperation {
    state: "Sent" | "Mined" | "Committed" | "Verified";
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
        await this.awaitEthereumTxCommit();
        if (this.state != "Mined") return;
        const receipt = await this.zkSyncProvider.notifyPriorityOp(
            this.priorityOpId.toNumber(),
            "COMMIT"
        );
        
        if (receipt.executed == false) {
            throw receipt;
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
        
        if (receipt.executed == false) {
            throw receipt;
        }

        this.state = "Verified";
        return receipt;
    }
}

class Transaction {
    state: "Sent" | "Committed" | "Verified";

    constructor(
        public txData,
        public txHash: string,
        public sidechainProvider: Provider
    ) {
        this.state = "Sent";
    }

    async awaitReceipt(): Promise<TransactionReceipt> {
        if (this.state !== "Sent") return;

        const receipt = await this.sidechainProvider.notifyTransaction(
            this.txHash,
            "COMMIT"
        );
        this.state = "Committed";

        if (!receipt.success) {
            throw receipt;
        }

        return receipt;
    }

    async awaitVerifyReceipt(): Promise<TransactionReceipt> {
        await this.awaitReceipt();
        const receipt = await this.sidechainProvider.notifyTransaction(
            this.txHash,
            "VERIFY"
        );
        this.state = "Verified";
        
        if (!receipt.success) {
            throw receipt;
        }
        
        return receipt;
    }
}
