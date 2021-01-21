import { BigNumber, BigNumberish, ethers } from 'ethers';
import {
    Address,
    TokenLike,
    Nonce,
    ChangePubKey,
    ChangePubKeyFee,
    SignedTransaction,
    TxEthSignature,
    ZkSyncVersion
} from './types';
import { getChangePubkeyMessage, serializeTx } from './utils';
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

/**
 * Provides iterface for constructing batches of transactions.
 */
export class BatchBuilder {
    private changePubKeyTx: ChangePubKey = null;
    private changePubKeyOnChain: boolean = null;

    private constructor(
        private wallet: Wallet,
        private nonce: Nonce,
        private txs: InternalTx[] = [],
        public zkSyncVersion: ZkSyncVersion
    ) {}

    static fromWallet(wallet: Wallet, nonce?: Nonce, version?: ZkSyncVersion): BatchBuilder {
        if (version == null) {
            version = 'contracts-3';
        }
        const batchBuilder = new BatchBuilder(wallet, nonce, [], version);
        return batchBuilder;
    }

    /**
     * Construct the batch from the given transactions.
     * Returs it with the corresponding Ethereum signature and total fee.
     * The message signed is keccak256(batchBytes) possibly prefixed with ChangePubKeyMessage if it's in the batch.
     * @param feeToken If provided, the fee for the whole batch will be obtained from the server in this token.
     * Possibly creates phantom transfer.
     */
    async build(
        feeToken?: TokenLike
    ): Promise<{ txs: SignedTransaction[]; signature: TxEthSignature; totalFee: BigNumber }> {
        if (this.txs.length == 0) {
            throw new Error('Transaction batch cannot be empty');
        }
        if (feeToken != undefined) {
            await this.setFeeToken(feeToken);
        }
        const totalFee = this.txs
            .map((tx) => tx.tx.fee)
            .reduce((sum: BigNumber, current: BigNumber) => sum.add(current), BigNumber.from(0));
        const { txs, bytes } = await this.processTransactions();

        const batchHash = ethers.utils.keccak256(bytes);
        let signature: TxEthSignature;
        if (this.changePubKeyOnChain === false) {
            // The message is ChangePubKeyMessage + keccak256(batchBytes).
            // Used for both batch and ChangePubKey transaction.
            signature = await this.wallet.getEthMessageSignature(
                getChangePubkeyMessage(
                    this.changePubKeyTx.newPkHash,
                    this.changePubKeyTx.nonce,
                    this.wallet.accountId,
                    batchHash
                )
            );
            // It is necessary to store the hash, so the signature can be verified on smart contract.
            this.changePubKeyTx.ethAuthData = {
                type: 'ECDSA',
                ethSignature: signature.signature,
                batchHash
            };
        } else {
            // The message is just keccak256(batchBytes).
            signature = await this.wallet.getEthMessageSignature(
                Uint8Array.from(Buffer.from(batchHash.slice(2), 'hex'))
            );
            if (this.changePubKeyTx != null) {
                this.changePubKeyTx.ethAuthData = {
                    type: 'Onchain'
                };
            }
        }

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
    }): BatchBuilder {
        const fee = withdraw.fee != undefined ? withdraw.fee : 0;
        const _withdraw = {
            ethAddress: withdraw.ethAddress,
            token: withdraw.token,
            amount: withdraw.amount,
            fee: fee,
            nonce: null
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

    addTransfer(transfer: { to: Address; token: TokenLike; amount: BigNumberish; fee?: BigNumberish }): BatchBuilder {
        const fee = transfer.fee != undefined ? transfer.fee : 0;
        const _transfer = {
            to: transfer.to,
            token: transfer.token,
            amount: transfer.amount,
            fee: fee,
            nonce: null
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

    addChangePubKey(changePubKey: { feeToken: TokenLike; fee?: BigNumberish; onchainAuth?: boolean }): BatchBuilder {
        if (this.changePubKeyOnChain != null) {
            throw new Error('ChangePubKey operation must be unique within a batch');
        }
        const fee = changePubKey.fee != undefined ? changePubKey.fee : 0;
        const onchainAuth = changePubKey.onchainAuth != undefined ? changePubKey.onchainAuth : false;
        this.changePubKeyOnChain = onchainAuth;
        const _changePubKey = {
            feeToken: changePubKey.feeToken,
            fee: fee,
            nonce: null,
            onchainAuth: onchainAuth
        };
        const feeType = {
            ChangePubKey: {
                onchainPubkeyAuth: _changePubKey.onchainAuth
            }
        };
        this.txs.push({
            type: 'ChangePubKey',
            tx: _changePubKey,
            feeType: feeType,
            address: this.wallet.address(),
            token: _changePubKey.feeToken
        });
        return this;
    }

    addForcedExit(forcedExit: { target: Address; token: TokenLike; fee?: BigNumberish }): BatchBuilder {
        const fee = forcedExit.fee != undefined ? forcedExit.fee : 0;
        const _forcedExit = {
            target: forcedExit.target,
            token: forcedExit.token,
            fee: fee,
            nonce: null
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
     * Sets transactions nonces, assembles the batch and serializes them into single array.
     */
    private async processTransactions(): Promise<{ txs: SignedTransaction[]; bytes: Uint8Array }> {
        const processedTxs: SignedTransaction[] = [];
        const _bytes: Uint8Array[] = [];
        let nonce: number = await this.wallet.getNonce(this.nonce);
        for (const tx of this.txs) {
            tx.tx.nonce = nonce++;
            switch (tx.type) {
                case 'Withdraw':
                    const withdraw = { tx: await this.wallet.getWithdrawFromSyncToEthereum(tx.tx) };
                    _bytes.push(serializeTx(withdraw.tx, this.zkSyncVersion));
                    processedTxs.push(withdraw);
                    break;
                case 'Transfer':
                    const transfer = { tx: await this.wallet.getTransfer(tx.tx) };
                    _bytes.push(serializeTx(transfer.tx, this.zkSyncVersion));
                    processedTxs.push(transfer);
                    break;
                case 'ChangePubKey':
                    const changePubKey = { tx: await this.wallet.getChangePubKey(tx.tx) };
                    const currentPubKeyHash = await this.wallet.getCurrentPubKeyHash();
                    if (currentPubKeyHash === changePubKey.tx.newPkHash) {
                        throw new Error('Current signing key is already set');
                    }
                    // We will sign it if necessary and store the batch hash.
                    this.changePubKeyTx = changePubKey.tx;
                    _bytes.push(serializeTx(changePubKey.tx, this.zkSyncVersion));
                    processedTxs.push(changePubKey);
                    break;
                case 'ForcedExit':
                    const forcedExit = { tx: await this.wallet.getForcedExit(tx.tx) };
                    _bytes.push(serializeTx(forcedExit.tx, this.zkSyncVersion));
                    processedTxs.push(forcedExit);
                    break;
            }
        }
        return {
            txs: processedTxs,
            bytes: ethers.utils.concat(_bytes)
        };
    }
}
