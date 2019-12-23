import { Contract, ContractTransaction, ethers, utils } from "ethers";
import { ETHProxy, Provider } from "./provider";
import { Signer } from "./signer";
import {
    AccountState,
    Address,
    Token,
    Nonce,
    PriorityOperationReceipt,
    TransactionReceipt
} from "./types";
import {
    IERC20_INTERFACE,
    SYNC_MAIN_CONTRACT_INTERFACE,
    SYNC_PRIOR_QUEUE_INTERFACE
} from "./utils";
import { serializePointPacked } from "./crypto";

export class Wallet {
    public provider: Provider;
    public ethProxy: ETHProxy;

    constructor(
        public signer: Signer,
    ) {}

    connect(provider: Provider, ethProxy: ETHProxy) {
        this.provider = provider;
        this.ethProxy = ethProxy;
        return this;
    }

    async syncTransfer(transfer: {
        to: Address;
        token: Token;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction> {
        const tokenId = await this.ethProxy.resolveTokenId(transfer.token);
        const nonce =
            transfer.nonce != null
                ? await this.getNonce(transfer.nonce)
                : await this.getNonce();
        const transactionData = {
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
        ethAddress: string;
        token: Token;
        amount: utils.BigNumberish;
        fee: utils.BigNumberish;
        nonce?: Nonce;
    }): Promise<Transaction> {
        const tokenId = await this.ethProxy.resolveTokenId(withdraw.token);
        const nonce =
            withdraw.nonce == null
                ? await this.getNonce(withdraw.nonce)
                : await this.getNonce();
        const transactionData = {
            ethAddress: withdraw.ethAddress,
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

    async getNonce(nonce: Nonce = "committed"): Promise<number> {
        if (nonce == "committed") {
            return (await this.provider.getState(this.signer.address()))
                .committed.nonce;
        } else if (typeof nonce == "number") {
            return nonce;
        }
    }

    address(): Address {
        return this.signer.address();
    }

    static async fromEthSigner(
        ethWallet: ethers.Signer,
        provider?: Provider,
        ethProxy?: ETHProxy
    ): Promise<Wallet> {
        const seedHex = (await ethWallet.signMessage("Matter login")).substr(2);
        const seed = Buffer.from(seedHex, "hex");
        const signer = Signer.fromSeed(seed);
        const wallet = new Wallet(signer);
        if (provider && ethProxy) {
            wallet.connect(provider, ethProxy);
        }
        return wallet;
    }

    async getAccountState(): Promise<AccountState> {
        return this.provider.getState(this.signer.address());
    }

    async getBalance(
        token: Token,
        type: "committed" | "verified" = "committed"
    ): Promise<utils.BigNumber> {
        const accountState = await this.getAccountState();
        if (token != "ETH") {
            token = token.toLowerCase();
        }
        let balance;
        if (type == "committed") {
            balance = accountState.committed.balances[token] || "0";
        } else {
            balance = accountState.verified.balances[token] || "0";
        }
        return utils.bigNumberify(balance);
    }
}

export async function depositFromETH(deposit: {
    depositFrom: ethers.Signer;
    depositTo: Wallet;
    token: Token;
    amount: utils.BigNumberish;
    maxFeeInETHToken?: utils.BigNumberish;
}): Promise<ETHOperation> {
    const gasPrice = await deposit.depositFrom.provider.getGasPrice();
    
    let maxFeeInETHToken;    
    if (deposit.maxFeeInETHToken != null) {
        maxFeeInETHToken = deposit.maxFeeInETHToken;
    } else {
        const baseFee = await deposit.depositTo.ethProxy.estimateDepositFeeInETHToken(
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

    if (deposit.token == "ETH") {
        ethTransaction = await mainZkSyncContract.depositETH(
            deposit.amount,
            deposit.depositTo.address().replace('sync:', '0x'),
            {
                value: utils.bigNumberify(deposit.amount).add(maxFeeInETHToken),
                gasLimit: utils.bigNumberify("200000"),
                gasPrice,
            }
        );
    } else {
        // ERC20 token deposit
        const erc20contract = new Contract(
            deposit.token,
            IERC20_INTERFACE,
            deposit.depositFrom
        );
        const approveTx = await erc20contract.approve(
            deposit.depositTo.provider.contractAddress.mainContract,
            deposit.amount
        );
        ethTransaction = await mainZkSyncContract.depositERC20(
            deposit.token,
            deposit.amount,
            deposit.depositTo.address().replace('sync:', '0x'),
            {
                gasLimit: utils.bigNumberify("250000"),
                value: maxFeeInETHToken,
                nonce: approveTx.nonce + 1,
                gasPrice,
            }
        );
    }

    return new ETHOperation(ethTransaction, deposit.depositTo.provider);
}

export async function emergencyWithdraw(withdraw: {
    withdrawTo: ethers.Signer;
    withdrawFrom: Wallet;
    token: Token;
    maxFeeInETHToken?: utils.BigNumberish;
    accountId?: number;
    nonce?: Nonce;
}): Promise<ETHOperation> {
    const gasPrice = await withdraw.withdrawTo.provider.getGasPrice();

    let maxFeeInETHToken;
    if (withdraw.maxFeeInETHToken != null) {
        maxFeeInETHToken = withdraw.maxFeeInETHToken;
    } else {
        const baseFee = await withdraw.withdrawFrom.ethProxy
            .estimateEmergencyWithdrawFeeInETHToken(gasPrice);

        maxFeeInETHToken = baseFee;
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

    const tokenId = await withdraw.withdrawFrom.ethProxy.resolveTokenId(
        withdraw.token
    );
    const nonce =
        withdraw.nonce != null
            ? await withdraw.withdrawFrom.getNonce(withdraw.nonce)
            : await withdraw.withdrawFrom.getNonce();
    const emergencyWithdrawSignature = withdraw.withdrawFrom.signer.syncEmergencyWithdrawSignature(
        {
            accountId,
            ethAddress: await withdraw.withdrawTo.getAddress(),
            tokenId,
            nonce
        }
    );

    const mainZkSyncContract = new Contract(
        withdraw.withdrawFrom.ethProxy.contractAddress.mainContract,
        SYNC_MAIN_CONTRACT_INTERFACE,
        withdraw.withdrawTo
    );

    let tokenAddress = "0x0000000000000000000000000000000000000000";
    if (withdraw.token != "ETH") {
        tokenAddress = withdraw.token;
    }
    const ethTransaction = await mainZkSyncContract.fullExit(
        accountId,
        serializePointPacked(withdraw.withdrawFrom.signer.publicKey),
        tokenAddress,
        emergencyWithdrawSignature,
        nonce,
        {
            gasLimit: utils.bigNumberify("500000"),
            value: maxFeeInETHToken,
            gasPrice,
        }
    );

    return new ETHOperation(ethTransaction, withdraw.withdrawFrom.provider);
}

export async function getEthereumBalance(
    ethSigner: ethers.Signer,
    token: Token
): Promise<utils.BigNumber> {
    let balance: utils.BigNumber;
    if (token == "ETH") {
        balance = await ethSigner.provider.getBalance(
            await ethSigner.getAddress()
        );
    } else {
        const erc20contract = new Contract(token, IERC20_INTERFACE, ethSigner);
        balance = await erc20contract.balanceOf(await ethSigner.getAddress());
    }
    return balance;
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
            const priorityQueueLog = SYNC_PRIOR_QUEUE_INTERFACE.parseLog(log);
            if (priorityQueueLog) {
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
