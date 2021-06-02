import { BigNumber, BigNumberish } from 'ethers';

// 0x-prefixed, hex encoded, ethereum account address
export type Address = string;
// sync:-prefixed, hex encoded, hash of the account public key
export type PubKeyHash = string;

// Symbol like "ETH" or "FAU" or token contract address(zero address is implied for "ETH").
export type TokenLike = TokenSymbol | TokenAddress | number;
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

export interface NFT {
    id: number;
    symbol: string;
    creatorId: number;
    serialId: number;
    address: string;
    creatorAddress: string;
    contentHash: string;
}

export interface AccountState {
    address: Address;
    id?: number;
    depositing: {
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
        nfts: {
            // NFT are indexed by their id
            [tokenId: number]: NFT;
        };
        mintedNfts: {
            // NFT are indexed by their id
            [tokenId: number]: NFT;
        };
        nonce: number;
        pubKeyHash: PubKeyHash;
    };
    verified: {
        balances: {
            // Token are indexed by their symbol (e.g. "ETH")
            [token: string]: BigNumberish;
        };
        nfts: {
            // NFT are indexed by their id
            [tokenId: number]: NFT;
        };
        mintedNfts: {
            // NFT are indexed by their id
            [tokenId: number]: NFT;
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

export type Ratio = [BigNumberish, BigNumberish];

/// represents ratio between tokens themself
export type TokenRatio = {
    type: 'Token';
    [token: string]: string | number;
    [token: number]: string | number;
};

/// represents ratio between lowest token denominations (wei, satoshi, etc.)
export type WeiRatio = {
    type: 'Wei';
    [token: string]: BigNumberish;
    [token: number]: BigNumberish;
};

export interface Order {
    accountId: number;
    recipient: Address;
    nonce: number;
    tokenSell: number;
    tokenBuy: number;
    ratio: Ratio;
    amount: BigNumberish;
    signature?: Signature;
    ethSignature?: TxEthSignature;
    validFrom: number;
    validUntil: number;
}

export interface Swap {
    type: 'Swap';
    orders: [Order, Order];
    amounts: [BigNumberish, BigNumberish];
    submitterId: number;
    submitterAddress: Address;
    nonce: number;
    signature?: Signature;
    feeToken: number;
    fee: BigNumberish;
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

export interface MintNFT {
    type: 'MintNFT';
    creatorId: number;
    creatorAddress: Address;
    recipient: Address;
    contentHash: string;
    fee: BigNumberish;
    feeToken: number;
    nonce: number;
    signature?: Signature;
}

export interface WithdrawNFT {
    type: 'WithdrawNFT';
    accountId: number;
    from: Address;
    to: Address;
    token: number;
    feeToken: number;
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

export type TxEthSignatureVariant = null | TxEthSignature | (TxEthSignature | null)[];

export interface SignedTransaction {
    tx: Transfer | Withdraw | ChangePubKey | CloseAccount | ForcedExit | MintNFT | WithdrawNFT | Swap;
    ethereumSignature?: TxEthSignatureVariant;
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
    feeType:
        | 'Withdraw'
        | 'Transfer'
        | 'TransferToNew'
        | 'FastWithdraw'
        | ChangePubKeyFee
        | 'MintNFT'
        | 'WithdrawNFT'
        | 'Swap';
    // Amount of gas used by transaction
    gasTxAmount: BigNumber;
    // Gas price (in wei)
    gasPriceWei: BigNumber;
    // Ethereum gas part of fee (in wei)
    gasFee: BigNumber;
    // Zero-knowledge proof part of fee (in wei)
    zkpFee: BigNumber;
    // Total fee amount (in wei)
    totalFee: BigNumber;
}

export interface BatchFee {
    // Total fee amount (in wei)
    totalFee: BigNumber;
}
