import { BigNumber, BigNumberish } from 'ethers';
import {
    Address,
    TokenLike,
    Nonce,
    ChangePubKey,
    ChangePubKeyFee,
    SignedTransaction,
    TxEthSignature,
    ChangePubkeyTypes
} from './types';
import { MAX_TIMESTAMP } from './utils';
import { Wallet } from './wallet';

/**
 * Used by `BatchBuilder` to store transactions until the `build()` call.
 */
interface InternalTx {
    type: 'Withdraw' | 'Transfer' | 'ChangePubKey' | 'ForcedExit';
    tx: any;
    feeType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee;
    address: Address;
    token: TokenLike;
}

type TotalFee = Map<TokenLike, BigNumber>;

/**
 * Provides iterface for constructing batches of transactions.
 */
export class BatchBuilder {
    private constructor(private wallet: Wallet, private nonce: Nonce, private txs: InternalTx[] = []) {}

    static fromWallet(wallet: Wallet, nonce?: Nonce): BatchBuilder {
        const batchBuilder = new BatchBuilder(wallet, nonce, []);
        return batchBuilder;
    }

    /**
     * Construct the batch from the given transactions.
     * Returs it with the corresponding Ethereum signature and total fee.
     * @param feeToken If provided, the fee for the whole batch will be obtained from the server in this token.
     * Possibly creates phantom transfer.
     */
    async build(
        feeToken?: TokenLike
    ): Promise<{ txs: SignedTransaction[]; signature: TxEthSignature; totalFee: TotalFee }> {
        if (this.txs.length == 0) {
            throw new Error('Transaction batch cannot be empty');
        }
        if (feeToken != undefined) {
            await this.setFeeToken(feeToken);
        }
        // Gather total fee for every token.
        const totalFee: TotalFee = new Map();
        for (const tx of this.txs) {
            const fee = tx.tx.fee;
            const token = tx.token;
            const curr: BigNumber = totalFee.get(token) || BigNumber.from(0);
            totalFee.set(token, curr.add(fee));
        }

        const { txs, message } = await this.processTransactions();

        let signature = await this.wallet.getEthMessageSignature(message);

        return {
            txs,
            signature,
            totalFee
        };
    }

    private async setFeeToken(feeToken: TokenLike) {
        // If user specified a token he wants to pay with, we expect all fees to be zero.
        if (this.txs.find((tx) => !BigNumber.from(tx.tx.fee).isZero()) != undefined) {
            throw new Error('Fees are expected to be zero');
        }
        let txWithFeeToken = this.txs.find((tx) => tx.token == feeToken);
        // If there's no transaction with the given token, create dummy transfer.
        if (txWithFeeToken == undefined) {
            this.addTransfer({
                to: this.wallet.address(),
                token: feeToken,
                amount: 0
            });
            txWithFeeToken = this.txs[this.txs.length - 1];
        }
        const txTypes = this.txs.map((tx) => tx.feeType);
        const addresses = this.txs.map((tx) => tx.address);

        txWithFeeToken.tx.fee = await this.wallet.provider.getTransactionsBatchFee(txTypes, addresses, feeToken);
    }

    addWithdraw(withdraw: {
        ethAddress: string;
        token: TokenLike;
        amount: BigNumberish;
        fee?: BigNumberish;
        fastProcessing?: boolean;
        validFrom?: number;
        validUntil?: number;
    }): BatchBuilder {
        const _withdraw = {
            ethAddress: withdraw.ethAddress,
            token: withdraw.token,
            amount: withdraw.amount,
            fee: withdraw.fee || 0,
            nonce: null,
            validFrom: withdraw.validFrom || 0,
            validUntil: withdraw.validUntil || MAX_TIMESTAMP
        };
        const feeType = withdraw.fastProcessing === true ? 'FastWithdraw' : 'Withdraw';
        this.txs.push({
            type: 'Withdraw',
            tx: _withdraw,
            feeType: feeType,
            address: _withdraw.ethAddress,
            token: _withdraw.token
        });
        return this;
    }

    addTransfer(transfer: {
        to: Address;
        token: TokenLike;
        amount: BigNumberish;
        fee?: BigNumberish;
        validFrom?: number;
        validUntil?: number;
    }): BatchBuilder {
        const _transfer = {
            to: transfer.to,
            token: transfer.token,
            amount: transfer.amount,
            fee: transfer.fee || 0,
            nonce: null,
            validFrom: transfer.validFrom || 0,
            validUntil: transfer.validUntil || MAX_TIMESTAMP
        };
        this.txs.push({
            type: 'Transfer',
            tx: _transfer,
            feeType: 'Transfer',
            address: _transfer.to,
            token: _transfer.token
        });
        return this;
    }

    addChangePubKey(changePubKey: {
        feeToken: TokenLike;
        ethAuthType: ChangePubkeyTypes;
        fee?: BigNumberish;
        validFrom?: number;
        validUntil?: number;
    }): BatchBuilder {
        const _changePubKey = {
            feeToken: changePubKey.feeToken,
            fee: changePubKey.fee || 0,
            nonce: null,
            ethAuthType: changePubKey.ethAuthType,
            validFrom: changePubKey.validFrom || 0,
            validUntil: changePubKey.validUntil || MAX_TIMESTAMP,
            pubKeyHash: null
        };
        const feeType = {
            ChangePubKey: changePubKey.ethAuthType
        };
        this.txs.push({
            type: 'ChangePubKey',
            tx: _changePubKey,
            feeType,
            address: this.wallet.address(),
            token: _changePubKey.feeToken
        });
        return this;
    }

    addForcedExit(forcedExit: {
        target: Address;
        token: TokenLike;
        fee?: BigNumberish;
        validFrom?: number;
        validUntil?: number;
    }): BatchBuilder {
        const _forcedExit = {
            target: forcedExit.target,
            token: forcedExit.token,
            fee: forcedExit.fee || 0,
            nonce: null,
            validFrom: forcedExit.validFrom || 0,
            validUntil: forcedExit.validUntil || MAX_TIMESTAMP
        };
        this.txs.push({
            type: 'ForcedExit',
            tx: _forcedExit,
            feeType: 'Withdraw',
            address: _forcedExit.target,
            token: _forcedExit.token
        });
        return this;
    }

    /**
     * Sets transactions nonces, assembles the batch and constructs the message to be signed by user.
     */
    private async processTransactions(): Promise<{ txs: SignedTransaction[]; message: string }> {
        const processedTxs: SignedTransaction[] = [];
        let messages: string[] = [];
        let nonce: number = await this.wallet.getNonce(this.nonce);
        const batchNonce = nonce;
        for (const tx of this.txs) {
            tx.tx.nonce = nonce++;
            switch (tx.type) {
                case 'Withdraw':
                    messages.push(this.wallet.getWithdrawEthMessagePart(tx.tx));
                    const withdraw = { tx: await this.wallet.getWithdrawFromSyncToEthereum(tx.tx) };
                    processedTxs.push(withdraw);
                    break;
                case 'Transfer':
                    messages.push(this.wallet.getTransferEthMessagePart(tx.tx));
                    const transfer = { tx: await this.wallet.getTransfer(tx.tx) };
                    processedTxs.push(transfer);
                    break;
                case 'ChangePubKey':
                    const changePubKey = await this.wallet.signSetSigningKey(tx.tx);
                    tx.tx.pubKeyHash = (changePubKey.tx as ChangePubKey).newPkHash;
                    const currentPubKeyHash = await this.wallet.getCurrentPubKeyHash();
                    if (currentPubKeyHash === tx.tx.pubKeyHash) {
                        throw new Error('Current signing key is already set');
                    }
                    messages.push(this.wallet.getChangePubKeyEthMessagePart(tx.tx));
                    processedTxs.push(changePubKey);
                    break;
                case 'ForcedExit':
                    messages.push(this.wallet.getForcedExitEthMessagePart(tx.tx));
                    const forcedExit = { tx: await this.wallet.getForcedExit(tx.tx) };
                    processedTxs.push(forcedExit);
                    break;
            }
        }
        messages.push(`Nonce: ${batchNonce}`);
        return {
            txs: processedTxs,
            message: messages.filter((part) => part.length != 0).join('\n')
        };
    }
}
