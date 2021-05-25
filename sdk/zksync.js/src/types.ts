import { BigNumber, BigNumberish } from 'ethers';

// 0x-prefixed, hex encoded, ethereum account address
export type Address = string;
// sync:-prefixed, hex encoded, hash of the account public key
export type PubKeyHash = string;

// Symbol like "ETH" or "FAU" or token contract address(zero address is implied for "ETH").
export type TokenLike = TokenSymbol | TokenAddress;
// Token symbol (e.g. "ETH", "FAU", etc.)
export type TokenSymbol = string;
// Token address (e.g. 0xde..ad for ERC20, or 0x00.00 for "ETH")
export type TokenAddress = string;

export type TotalFee = Map<TokenLike, BigNumber>;

export type Nonce = number | 'committed';

export type Network = 'localhost' | 'rinkeby' | 'ropsten' | 'mainnet' | 'rinkeby-beta' | 'ropsten-beta';

export interface Create2Data {
    creatorAddress: string;
    saltArg: string;
    codeHash: string;
}

export interface AccountState {
    address: Address;
    id?: number;
    // This field will be presented only if using RPC API.
    depositing?: {
        balances: {
            // Token are indexed by their symbol (e.g. "ETH")
            [token: string]: {
                // Sum of pending deposits for the token.
                amount: BigNumberish;
                // Value denoting the block number when the funds are expected
                // to be received by zkSync network.
                expectedAcceptBlock: number;
            };
        };
    };
    committed: {
        balances: {
            // Token are indexed by their symbol (e.g. "ETH")
            [token: string]: BigNumberish;
        };
        nonce: number;
        pubKeyHash: PubKeyHash;
    };
    verified: {
        balances: {
            // Token are indexed by their symbol (e.g. "ETH")
            [token: string]: BigNumberish;
        };
        nonce: number;
        pubKeyHash: PubKeyHash;
    };
}

export type EthSignerType = {
    verificationMethod: 'ECDSA' | 'ERC-1271';
    // Indicates if signer adds `\x19Ethereum Signed Message\n${msg.length}` prefix before signing message.
    // i.e. if false, we should add this prefix manually before asking to sign message
    isSignedMsgPrefixed: boolean;
};

export interface TxEthSignature {
    type: 'EthereumSignature' | 'EIP1271Signature';
    signature: string;
}

export interface Signature {
    pubKey: string;
    signature: string;
}

export interface Transfer {
    type: 'Transfer';
    accountId: number;
    from: Address;
    to: Address;
    token: number;
    amount: BigNumberish;
    fee: BigNumberish;
    nonce: number;
    signature?: Signature;
    validFrom: number;
    validUntil: number;
}

export interface Withdraw {
    type: 'Withdraw';
    accountId: number;
    from: Address;
    to: Address;
    token: number;
    amount: BigNumberish;
    fee: BigNumberish;
    nonce: number;
    signature?: Signature;
    validFrom: number;
    validUntil: number;
}

export interface ForcedExit {
    type: 'ForcedExit';
    initiatorAccountId: number;
    target: Address;
    token: number;
    fee: BigNumberish;
    nonce: number;
    signature?: Signature;
    validFrom: number;
    validUntil: number;
}

export type ChangePubkeyTypes = 'Onchain' | 'ECDSA' | 'CREATE2' | 'ECDSALegacyMessage';

export interface ChangePubKeyOnchain {
    type: 'Onchain';
}

export interface ChangePubKeyECDSA {
    type: 'ECDSA';
    ethSignature: string;
    batchHash?: string;
}

export interface ChangePubKeyCREATE2 {
    type: 'CREATE2';
    creatorAddress: string;
    saltArg: string;
    codeHash: string;
}

export interface ChangePubKey {
    type: 'ChangePubKey';
    accountId: number;
    account: Address;
    newPkHash: PubKeyHash;
    feeToken: number;
    fee: BigNumberish;
    nonce: number;
    signature?: Signature;
    ethAuthData?: ChangePubKeyOnchain | ChangePubKeyECDSA | ChangePubKeyCREATE2;
    ethSignature?: string;
    validFrom: number;
    validUntil: number;
}

export interface CloseAccount {
    type: 'Close';
    account: Address;
    nonce: number;
    signature: Signature;
}

export interface SignedTransaction {
    tx: Transfer | Withdraw | ChangePubKey | CloseAccount | ForcedExit;
    ethereumSignature?: TxEthSignature;
}

export interface BlockInfo {
    blockNumber: number;
    committed: boolean;
    verified: boolean;
}

export interface TransactionReceipt {
    executed: boolean;
    success?: boolean;
    failReason?: string;
    block?: BlockInfo;
}

export interface PriorityOperationReceipt {
    executed: boolean;
    block?: BlockInfo;
}

export interface ContractAddress {
    mainContract: string;
    govContract: string;
}

export interface Tokens {
    // Tokens are indexed by their symbol (e.g. "ETH")
    [token: string]: {
        address: string;
        id: number;
        symbol: string;
        decimals: number;
    };
}

// we have to ignore this because of a bug in prettier causes this exact block
// to have double semicolons inside
// prettier-ignore
export interface ChangePubKeyFee {
    // Note: Ignore, since it just looks more intuitive if `"ChangePubKey"` is kept as a string literal)
    // prettier-ignore
    // Denotes how authorization of operation is performed:
    // 'Onchain' if it's done by sending an Ethereum transaction,
    // 'ECDSA' if it's done by providing an Ethereum signature in zkSync transaction.
    // 'CREATE2' if it's done by providing arguments to restore account ethereum address according to CREATE2 specification.
    "ChangePubKey": ChangePubkeyTypes;
}

export interface LegacyChangePubKeyFee {
    ChangePubKey: {
        onchainPubkeyAuth: boolean;
    };
}

export interface Fee {
    // Operation type (amount of chunks in operation differs and impacts the total fee).
    // This field will be presented only if using RPC API.
    feeType?: 'Withdraw' | 'Transfer' | 'TransferToNew' | 'FastWithdraw' | ChangePubKeyFee;
    // Amount of gas used by transaction
    // This field will be presented only if using RPC API.
    gasTxAmount?: BigNumber;
    // Gas price (in wei)
    // This field will be presented only if using RPC API.
    gasPriceWei?: BigNumber;
    // Ethereum gas part of fee (in wei)
    gasFee: BigNumber;
    // Zero-knowledge proof part of fee (in wei)
    zkpFee: BigNumber;
    // Total fee amount (in wei)
    totalFee: BigNumber;
}

export interface BatchFee {
    // Ethereum gas part of fee (in wei)
    // This field will be presented only if using REST API.
    gasFee?: BigNumber;
    // Zero-knowledge proof part of fee (in wei)
    // This field will be presented only if using REST API.
    zkpFee?: BigNumber;
    // Total fee amount (in wei)
    totalFee: BigNumber;
}

export interface PaginationQuery<F> {
    from: F;
    limit: number;
    direction: 'newer' | 'older';
}

export interface Paginated<T, F> {
    list: T[];
    pagination: {
        from: F;
        limit: number;
        direction: 'newer' | 'older';
        count: number;
    };
}

export interface ApiBlockInfo {
    blockNumber: number;
    newStateRoot: string;
    blockSize: number;
    commitTxHash?: string;
    verifyTxHash?: string;
    committedAt?: string;
    finalizedAt?: string;
    status: 'queued' | 'committed' | 'finalized';
}

export interface ApiAccountInfo {
    accountId: number;
    address: Address;
    nonce: number;
    pubKeyHash: PubKeyHash;
    lastUpdateInBlock: number;
    balances: {
        [token: string]: BigNumber;
    };
}

export interface ApiConfig {
    network: Network;
    contract: Address;
    govContract: Address;
    depositConfirmations: number;
    zksyncVersion: 'contractV4';
    // TODO: server_version (ZKS-627)
}

export interface ApiFee {
    gasFee: BigNumber;
    zkpFee: BigNumber;
    totalFee: BigNumber;
}

export interface NetworkStatus {
    lastCommitted: number;
    finalized: number;
    totalTransactions: number;
    mempoolSize: number;
}

export interface TokenInfo {
    id: number;
    address: Address;
    symbol: string;
    decimals: number;
    enabledForFees: boolean;
}

export interface TokenPriceInfo {
    tokenId: number;
    tokenSymbol: string;
    priceIn: string;
    decimals: number;
    price: BigNumber;
}

export interface SubmitBatchResponse {
    transactionHashes: string[];
    batchHash: string;
}

export interface ApiL1TxReceipt {
    status: 'queued' | 'committed' | 'finalized';
    ethBlock: number;
    rollupBlock?: number;
    id: number;
}

export interface ApiL2TxReceipt {
    txHash: string;
    rollupBlock?: number;
    status: 'queued' | 'committed' | 'finalized' | 'rejected';
    failReason?: string;
}

export type ApiTxReceipt = ApiL1TxReceipt | ApiL2TxReceipt;

export interface WithdrawAndEthHash {
    type: 'Withdraw';
    accountId: number;
    from: Address;
    to: Address;
    token: number;
    amount: BigNumberish;
    fee: BigNumberish;
    nonce: number;
    signature?: Signature;
    validFrom: number;
    validUntil: number;
    ethTxHash?: string;
}

export interface ForcedExitAndEthHash {
    type: 'ForcedExit';
    initiatorAccountId: number;
    target: Address;
    token: number;
    fee: BigNumberish;
    nonce: number;
    signature?: Signature;
    validFrom: number;
    validUntil: number;
    ethTxHash?: string;
}

export interface ApiDeposit {
    type: 'Deposit';
    from: Address;
    tokenId: number;
    amount: BigNumber;
    to: Address;
    accountId?: number;
    ethHash: string;
    id: number;
    txHash: string;
}

export interface ApiFullExit {
    type: 'FullExit';
    accountId: number;
    tokenId: number;
    ethHash: string;
    id: number;
    txHash: string;
}

export type L2Tx = Transfer | Withdraw | ChangePubKey | ForcedExit | CloseAccount;

export type L2TxData = Transfer | WithdrawAndEthHash | ChangePubKey | ForcedExitAndEthHash | CloseAccount;

export type TransactionData = L2TxData | ApiDeposit | ApiFullExit;

export interface ApiTransaction {
    txHash: string;
    blockNumber?: number;
    op: TransactionData;
    status: 'queued' | 'committed' | 'finalized' | 'rejected';
    failReason?: string;
    createdAt?: string;
}

export interface ApiSignedTx {
    tx: ApiTransaction;
    ethSignature?: string;
}

export interface ApiBatchStatus {
    updatedAt: string;
    lastState: 'queued' | 'committed' | 'finalized' | 'rejected';
}

export interface ApiBatchData {
    batchHash: string;
    transactionHashes: string[];
    createdAt: string;
    batchStatus: ApiBatchStatus;
}

export interface BlockAndTxHash {
    blockNumber: number;
    txHash: string;
}

export interface PendingOpsRequest {
    address: Address;
    accountId?: number;
    serialId: number;
}

export interface AccountTxsRequest {
    address: Address;
    txHash: string;
}
