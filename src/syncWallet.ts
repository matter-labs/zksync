import { Contract, ContractTransaction, ethers, utils } from "ethers";
import { ETHProxy, SyncProvider } from "./provider";
import { SyncSigner } from "./signer";
import { SyncAccountState, SyncAddress, Token } from "./types";
import {
    IERC20_INTERFACE,
    SYNC_MAIN_CONTRACT_INTERFACE,
    SYNC_PRIOR_QUEUE_INTERFACE
} from "./utils";
import { serializePointPacked } from "./crypto";

export class SyncWallet {
    constructor(
        public signer: SyncSigner,
        public provider: SyncProvider,
        public ethProxy: ETHProxy
    ) {}

    async syncTransfer(
        to: SyncAddress,
        token: Token,
        amount: utils.BigNumberish,
        fee: utils.BigNumberish,
        nonce: "committed" | number = "committed"
    ): Promise<TransactionHandle> {
        const tokenId = await this.ethProxy.resolveTokenId(token);
        const transaxtionData = {
            to,
            tokenId,
            amount,
            fee,
            nonce: await this.getNonce(nonce)
        };
        const signedTransferTransaction = this.signer.signSyncTransfer(
            transaxtionData
        );

        const transactionHash = await this.provider.submitTx(
            signedTransferTransaction
        );
        return new TransactionHandle(
            signedTransferTransaction,
            transactionHash,
            this.provider
        );
    }

    async withdrawTo(
        ethAddress: string,
        token: Token,
        amount: utils.BigNumberish,
        fee: utils.BigNumberish,
        nonce: "committed" | number = "committed"
    ): Promise<TransactionHandle> {
        const tokenId = await this.ethProxy.resolveTokenId(token);
        const transactionData = {
            ethAddress,
            tokenId,
            amount,
            fee,
            nonce: await this.getNonce(nonce)
        };
        const signedWithdrawTransaction = this.signer.signSyncWithdraw(
            transactionData
        );

        const submitResponse = await this.provider.submitTx(
            signedWithdrawTransaction
        );
        return new TransactionHandle(
            signedWithdrawTransaction,
            submitResponse,
            this.provider
        );
    }

    async close(
        nonce: "committed" | number = "committed"
    ): Promise<TransactionHandle> {
        const signerdCloseTransaction = this.signer.signSyncCloseAccount({
            nonce: await this.getNonce()
        });

        const transactionHash = await this.provider.submitTx(
            signerdCloseTransaction
        );
        return new TransactionHandle(
            signerdCloseTransaction,
            transactionHash,
            this.provider
        );
    }

    async getNonce(nonce: "committed" | number = "committed"): Promise<number> {
        if (nonce == "committed") {
            return (await this.provider.getState(this.signer.address()))
                .committed.nonce;
        } else if (typeof nonce == "number") {
            return nonce;
        }
    }

    address(): SyncAddress {
        return this.signer.address();
    }

    static async fromEthWallet(
        ethWallet: ethers.Signer,
        provider: SyncProvider,
        ethProxy: ETHProxy
    ): Promise<SyncWallet> {
        const seedHex = (await ethWallet.signMessage("Matter login")).substr(2);
        const seed = Buffer.from(seedHex, "hex");
        const signer = SyncSigner.fromSeed(seed);
        return new SyncWallet(signer, provider, ethProxy);
    }

    async getAccountState(): Promise<SyncAccountState> {
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

export async function depositFromETH(
    depositFrom: ethers.Signer,
    depositTo: SyncWallet,
    token: Token,
    amount: utils.BigNumberish,
    maxFeeInETHCurrenty: utils.BigNumberish
): Promise<ETHOperationHandle> {
    const mainSidechainContract = new Contract(
        depositTo.provider.contractAddress.mainContract,
        SYNC_MAIN_CONTRACT_INTERFACE,
        depositFrom
    );

    let ethTransaction;

    if (token == "ETH") {
        ethTransaction = await mainSidechainContract.depositETH(
            amount,
            depositTo.address(),
            {
                value: utils.bigNumberify(amount).add(maxFeeInETHCurrenty),
                gasLimit: utils.bigNumberify("200000")
            }
        );
    } else {
        // ERC20 token deposit
        const erc20contract = new Contract(
            token,
            IERC20_INTERFACE,
            depositFrom
        );
        const approveTx = await erc20contract.approve(
            depositTo.provider.contractAddress.mainContract,
            amount
        );
        ethTransaction = await mainSidechainContract.depositERC20(
            token,
            amount,
            depositTo.address(),
            {
                gasLimit: utils.bigNumberify("250000"),
                value: maxFeeInETHCurrenty,
                nonce: approveTx.nonce + 1
            }
        );
    }

    return new ETHOperationHandle(ethTransaction, depositTo.provider);
}

export async function emergencyWithdraw(
    withdrawTo: ethers.Signer,
    withdrawFrom: SyncWallet,
    token: Token,
    maxFeeInETHCurrenty: utils.BigNumberish,
    nonce: "committed" | number = "committed"
): Promise<ETHOperationHandle> {
    const tokenId = await withdrawFrom.ethProxy.resolveTokenId(token);
    const numNonce = await withdrawFrom.getNonce(nonce);
    const emergencyWithdrawSignature = withdrawFrom.signer.syncEmergencyWithdrawSignature(
        {
            ethAddress: await withdrawTo.getAddress(),
            tokenId,
            nonce: numNonce
        }
    );

    const mainSyncContract = new Contract(
        withdrawFrom.provider.contractAddress.mainContract,
        SYNC_MAIN_CONTRACT_INTERFACE,
        withdrawTo
    );

    let tokenAddress = "0x0000000000000000000000000000000000000000";
    if (token != "ETH") {
        tokenAddress = token;
    }
    const ethTransaction = await mainSyncContract.fullExit(
        serializePointPacked(withdrawFrom.signer.publicKey),
        tokenAddress,
        emergencyWithdrawSignature,
        numNonce,
        {
            gasLimit: utils.bigNumberify("500000"),
            value: maxFeeInETHCurrenty
        }
    );

    return new ETHOperationHandle(ethTransaction, withdrawFrom.provider);
}

class ETHOperationHandle {
    state: "Sent" | "Mined" | "Commited" | "Verified";
    priorityOpId?: utils.BigNumber;

    constructor(
        public ethTx: ContractTransaction,
        public sidechainProvider: SyncProvider
    ) {
        this.state = "Sent";
    }

    async waitTxMine() {
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
    }

    async waitCommit() {
        await this.waitTxMine();
        if (this.state != "Mined") return;
        await this.sidechainProvider.notifyPriorityOp(
            this.priorityOpId.toNumber(),
            "COMMIT"
        );
        this.state = "Commited";
    }

    async waitVerify() {
        await this.waitCommit();
        if (this.state != "Commited") return;

        await this.sidechainProvider.notifyPriorityOp(
            this.priorityOpId.toNumber(),
            "VERIFY"
        );
        this.state = "Verified";
    }
}

class TransactionHandle {
    state: "Sent" | "Commited" | "Verified";

    constructor(
        public txData,
        public txHash: string,
        public sidechainProvider: SyncProvider
    ) {
        this.state = "Sent";
    }

    async waitCommit() {
        if (this.state !== "Sent") return;

        await this.sidechainProvider.notifyTransaction(this.txHash, "COMMIT");
        this.state = "Commited";
    }

    async waitVerify() {
        await this.waitCommit();
        await this.sidechainProvider.notifyTransaction(this.txHash, "VERIFY");
        this.state = "Verified";
    }
}
