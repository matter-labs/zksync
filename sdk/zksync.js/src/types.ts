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

export type Nonce = number | 'committed';

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
    signature: Signature;
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
    signature: Signature;
}

export interface ForcedExit {
    type: 'ForcedExit';
    initiatorAccountId: number;
    target: Address;
    token: number;
    fee: BigNumberish;
    nonce: number;
    signature: Signature;
}

export interface ChangePubKey {
    type: 'ChangePubKey';
    accountId: number;
    account: Address;
    newPkHash: PubKeyHash;
    feeToken: number;
    fee: BigNumberish;
    nonce: number;
    signature: Signature;
    ethSignature: string;
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

export interface ChangePubKeyFee {
    // Note: Ignore, since it just looks more intuitive if `"ChangePubKey"` is kept as a string literal)
    // prettier-ignore
    "ChangePubKey": {
        // Denotes how authorization of operation is performed:
        // 'true' if it's done by sending an Ethereum transaction,
        // 'false' if it's done by providing an Ethereum signature in zkSync transaction.
        onchainPubkeyAuth: boolean;
    };
}

export interface Fee {
    // Operation type (amount of chunks in operation differs and impacts the total fee).
    feeType: 'Withdraw' | 'Transfer' | 'TransferToNew' | 'FastWithdraw' | ChangePubKeyFee;
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
