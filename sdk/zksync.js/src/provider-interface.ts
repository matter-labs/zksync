import {
    AccountState,
    Address,
    ChangePubKeyFee,
    ContractAddress,
    Fee,
    LegacyChangePubKeyFee,
    PriorityOperationReceipt,
    TokenLike,
    Tokens,
    TransactionReceipt,
    TxEthSignature
} from './types';
import { BigNumber } from 'ethers';
import { TokenSet } from './utils';

export abstract class SyncProvider {
    contractAddress: ContractAddress;
    public tokenSet: TokenSet;
    public providerType: 'RPC' | 'Rest';
    // For HTTP provider
    public pollIntervalMilliSecs = 500;

    abstract submitTx(tx: any, signature?: TxEthSignature, fastProcessing?: boolean): Promise<string>;
    abstract submitTxsBatch(
        transactions: { tx: any; signature?: TxEthSignature }[],
        ethSignatures?: TxEthSignature | TxEthSignature[]
    ): Promise<string[]>;
    abstract getContractAddress(): Promise<ContractAddress>;
    abstract getTokens(): Promise<Tokens>;
    abstract getState(address: Address): Promise<AccountState>;
    abstract getTxReceipt(txHash: string): Promise<TransactionReceipt>;
    abstract getPriorityOpStatus(hashOrSerialId: string | number): Promise<PriorityOperationReceipt>;
    abstract getConfirmationsForEthOpAmount(): Promise<number>;
    abstract notifyPriorityOp(
        hashOrSerialId: string | number,
        action: 'COMMIT' | 'VERIFY'
    ): Promise<PriorityOperationReceipt>;
    abstract notifyTransaction(hash: string, action: 'COMMIT' | 'VERIFY'): Promise<TransactionReceipt>;
    abstract getTransactionFee(
        txType: 'Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee,
        address: Address,
        tokenLike: TokenLike
    ): Promise<Fee>;
    abstract getTransactionsBatchFee(
        txTypes: ('Withdraw' | 'Transfer' | 'FastWithdraw' | ChangePubKeyFee | LegacyChangePubKeyFee)[],
        addresses: Address[],
        tokenLike: TokenLike
    ): Promise<BigNumber>;
    abstract getTokenPrice(tokenLike: TokenLike): Promise<number>;
    abstract getEthTxForWithdrawal(withdrawal_hash: string): Promise<string>;

    async updateTokenSet(): Promise<void> {
        const updatedTokenSet = new TokenSet(await this.getTokens());
        this.tokenSet = updatedTokenSet;
    }
    async disconnect() {}
}
