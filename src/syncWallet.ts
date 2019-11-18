import { Contract, ContractTransaction, ethers, utils } from "ethers";
import { ETHProxy, SyncProvider } from "./provider";
import { SyncSigner } from "./signer";
import { SyncAccountState, SyncAddress, Token } from "./types";
import {
    IERC20_INTERFACE,
    SYNC_MAIN_CONTRACT_INTERFACE,
    SYNC_PRIOR_QUEUE_INTERFACE
} from "./utils";

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
        nonce: "commited" | number = "commited"
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
        nonce: "commited" | number = "commited"
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
        nonce: "commited" | number = "commited"
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

    async getNonce(nonce: "commited" | number = "commited"): Promise<number> {
        if (nonce == "commited") {
            return (await this.provider.getState(this.signer.address()))
                .commited.nonce;
        } else if (typeof nonce == "number") {
            return nonce;
        }
    }

    address(): SyncAddress {
        return this.signer.address();
    }

    static async fromEthWallet(
        ethWallet: ethers.Signer,
        sidechainProvider: SyncProvider,
        ethProxy: ETHProxy
    ) {
        const seedHex = (await ethWallet.signMessage("Matter login")).substr(2);
        const seed = Buffer.from(seedHex, "hex");
        const signer = SyncSigner.fromSeed(seed);
        return new SyncWallet(signer, sidechainProvider, ethProxy);
    }

    async getAccountState(): Promise<SyncAccountState> {
        return this.provider.getState(this.signer.address());
    }

    async getBalance(
        token: Token,
        type: "commited" | "verified" = "commited"
    ): Promise<utils.BigNumber> {
        const accountState = await this.getAccountState();
        if (token != "ETH") {
            token = token.toLowerCase();
        }
        let balance;
        if (type == "commited") {
            balance = accountState.commited.balances[token] || "0";
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
) {
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

    return new DepositTransactionHandle(ethTransaction, depositTo.provider);
}

class DepositTransactionHandle {
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
