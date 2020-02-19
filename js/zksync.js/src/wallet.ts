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
    isTokenETH,
    SYNC_MAIN_CONTRACT_INTERFACE,
    SYNC_PRIOR_QUEUE_INTERFACE,
    TokenSet
} from "./utils";
import { serializePointPacked } from "./crypto";

export class Wallet {
    public provider: Provider;

    private constructor(
        public signer: Signer,
        public ethSigner: ethers.Signer,
        public cachedAddress: string,
        public tokensCache: TokenSet
    ) {}

    connect(provider: Provider) {
        this.provider = provider;
        return this;
    }

    async syncTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction> {
        const tokenId = await this.tokensCache.resolveTokenId(transfer.token);
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
        const signedTransferTransaction = this.signer.signSyncTransfer(
            transactionData
        );

        const transactionHash = await this.provider.submitTx(
            signedTransferTransaction
        );
        return new Transaction(
            signedTransferTransaction,
            transactionHash,
            this.provider
        );
    }

    async withdrawTo(withdraw: {
        ethAddress?: string;
        token: TokenLike;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction> {
        const withdrawAddress =
            withdraw.ethAddress == null ? this.address() : withdraw.ethAddress;
        const tokenId = await this.tokensCache.resolveTokenId(withdraw.token);
        const nonce =
            withdraw.nonce != null
                ? await this.getNonce(withdraw.nonce)
                : await this.getNonce();
        const transactionData = {
            from: this.address(),
            ethAddress: withdrawAddress,
            tokenId,
            amount: withdraw.amount,
            fee: withdraw.fee,
            nonce
        };
        const signedWithdrawTransaction = this.signer.signSyncWithdraw(
            transactionData
        );

        const submitResponse = await this.provider.submitTx(
            signedWithdrawTransaction
        );
        return new Transaction(
            signedWithdrawTransaction,
            submitResponse,
            this.provider
        );
    }

    async close(nonce: Nonce = "committed"): Promise<Transaction> {
        const numNonce = await this.getNonce(nonce);
        const signedCloseTransaction = this.signer.signSyncCloseAccount({
            nonce: numNonce
        });

        const transactionHash = await this.provider.submitTx(
            signedCloseTransaction
        );
        return new Transaction(
            signedCloseTransaction,
            transactionHash,
            this.provider
        );
    }

    async isCurrentPubkeySet(): Promise<boolean> {
        const currentPubKeyHash = await this.getCurrentPubKeyHash();
        const signerPubKeyHash = this.signer.pubKeyHash();
        return currentPubKeyHash === signerPubKeyHash;
    }

    async setCurrentPubkeyWithZksyncTx(
        nonce: Nonce = "committed",
        onchainAuth = false
    ): Promise<Transaction> {
        const currentPubKeyHash = await this.getCurrentPubKeyHash();
        const newPubKeyHash = this.signer.pubKeyHash();

        if (currentPubKeyHash == newPubKeyHash) {
            throw new Error("Current PubKeyHash is the same as new");
        }

        const numNonce = await this.getNonce(nonce);
        const newPkHash = serializeAddress(newPubKeyHash);
        const message = Buffer.concat([serializeNonce(numNonce), newPkHash]);
        const ethSignature = onchainAuth
            ? null
            : await this.ethSigner.signMessage(message);

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

    async authChangePubkey(
        nonce: Nonce = "committed"
    ): Promise<ContractTransaction> {
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
                gasLimit: utils.bigNumberify("200000")
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

    static async fromEthSigner(
        ethWallet: ethers.Signer,
        provider: Provider
    ): Promise<Wallet> {
        const seedHex = (await ethWallet.signMessage("Matter login")).substr(2);
        const seed = Buffer.from(seedHex, "hex");
        const signer = Signer.fromSeed(seed);
        const tokenCache = new TokenSet(await provider.getTokens());
        const wallet = new Wallet(
            signer,
            ethWallet,
            await ethWallet.getAddress(),
            tokenCache
        );
        wallet.connect(provider);
        return wallet;
    }

    async getAccountState(): Promise<AccountState> {
        return this.provider.getState(this.address());
    }

    async getBalance(
        token: TokenLike,
        type: "committed" | "verified" = "committed"
    ): Promise<utils.BigNumber> {
        const accountState = await this.getAccountState();
        const tokenSymbol = this.tokensCache.resolveTokenSymbol(token);
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
                this.tokensCache.resolveTokenAddress(token),
                IERC20_INTERFACE,
                this.ethSigner
            );
            balance = await erc20contract.balanceOf(this.cachedAddress);
        }
        return balance;
    }
}

export async function depositFromETH(deposit: {
    depositFrom: ethers.Signer;
    depositTo: Wallet;
    token: TokenLike;
    amount: utils.BigNumberish;
    maxFeeInETHToken?: utils.BigNumberish;
}): Promise<ETHOperation> {
    const gasPrice = await deposit.depositFrom.provider.getGasPrice();

    const ethProxy = new ETHProxy(
        deposit.depositFrom.provider,
        deposit.depositTo.provider.contractAddress
    );

    let maxFeeInETHToken;
    if (deposit.maxFeeInETHToken != null) {
        maxFeeInETHToken = deposit.maxFeeInETHToken;
    } else {
        const baseFee = await ethProxy.estimateDepositFeeInETHToken(
            deposit.token,
            gasPrice
        );
        maxFeeInETHToken = baseFee;
    }
    const mainZkSyncContract = new Contract(
        deposit.depositTo.provider.contractAddress.mainContract,
        SYNC_MAIN_CONTRACT_INTERFACE,
        deposit.depositFrom
    );

    let ethTransaction;

    if (isTokenETH(deposit.token)) {
        ethTransaction = await mainZkSyncContract.depositETH(
            deposit.amount,
            deposit.depositTo.address(),
            {
                value: utils.bigNumberify(deposit.amount).add(maxFeeInETHToken),
                gasLimit: utils.bigNumberify("200000"),
                gasPrice
            }
        );
    } else {
        const tokenAddress = deposit.depositTo.tokensCache.resolveTokenAddress(
            deposit.token
        );
        // ERC20 token deposit
        const erc20contract = new Contract(
            tokenAddress,
            IERC20_INTERFACE,
            deposit.depositFrom
        );
        const approveTx = await erc20contract.approve(
            deposit.depositTo.provider.contractAddress.mainContract,
            deposit.amount
        );
        ethTransaction = await mainZkSyncContract.depositERC20(
            tokenAddress,
            deposit.amount,
            deposit.depositTo.address(),
            {
                gasLimit: utils.bigNumberify("250000"),
                value: maxFeeInETHToken,
                nonce: approveTx.nonce + 1,
                gasPrice
            }
        );
    }

    return new ETHOperation(ethTransaction, deposit.depositTo.provider);
}

export async function emergencyWithdraw(withdraw: {
    withdrawFrom: Wallet;
    token: TokenLike;
    maxFeeInETHToken?: utils.BigNumberish;
    accountId?: number;
    nonce?: Nonce;
}): Promise<ETHOperation> {
    const gasPrice = await withdraw.withdrawFrom.ethSigner.provider.getGasPrice();
    const ethProxy = new ETHProxy(
        withdraw.withdrawFrom.ethSigner.provider,
        withdraw.withdrawFrom.provider.contractAddress
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
        const accountState = await withdraw.withdrawFrom.getAccountState();
        if (!accountState.id) {
            throw new Error("Can't resolve account id from the ZK Sync node");
        }
        accountId = accountState.id;
    }

    const mainZkSyncContract = new Contract(
        ethProxy.contractAddress.mainContract,
        SYNC_MAIN_CONTRACT_INTERFACE,
        withdraw.withdrawFrom.ethSigner
    );

    const tokenAddress = withdraw.withdrawFrom.tokensCache.resolveTokenAddress(
        withdraw.token
    );
    const ethTransaction = await mainZkSyncContract.fullExit(
        accountId,
        tokenAddress,
        {
            gasLimit: utils.bigNumberify("500000"),
            value: maxFeeInETHToken,
            gasPrice
        }
    );

    return new ETHOperation(ethTransaction, withdraw.withdrawFrom.provider);
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
}
