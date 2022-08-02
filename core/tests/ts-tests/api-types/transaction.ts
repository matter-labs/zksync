export interface ChangePubKeyOnchain {
    type: 'Onchain';
}

export interface ChangePubKeyECDSA {
    type: 'ECDSA';
    ethSignature: string;
    batchHash?: string;
}

export interface ChangePubKeyEIP712 {
    type: 'EIP712';
    ethSignature: string;
    batchHash?: string;
}

export interface ChangePubKeyCREATE2 {
    type: 'CREATE2';
    creatorAddress: string;
    saltArg: string;
    codeHash: string;
}

export type ChangePubKeyOp = {
    fee?: string;
    feeToken?: number;
    account: string;
    accountId: number;
    signature: {
        pubKey: string;
        signature: string;
    };
    ethAuthData: ChangePubKeyOnchain | ChangePubKeyECDSA | ChangePubKeyCREATE2 | ChangePubKeyEIP712;
    newPkHash: string;
    nonce: number;
    chainId?: number;
    type: 'ChangePubKey';
    ethSignature: null;
    validFrom: number;
    validUntil: number;
};

export type TransferOp = {
    amount: string;
    fee: string;
    from: string;
    accountId: number;
    nonce: number;
    signature: {
        pubKey: string;
        signature: string;
    };
    to: string;
    token: number;
    type: 'Transfer';
    validFrom: number;
    validUntil: number;
};

export type WithdrawOp = {
    amount: string;
    fee: string;
    from: string;
    accountId: number;
    nonce: number;
    signature: {
        pubKey: string;
        signature: string;
    };
    to: string;
    token: number;
    type: 'Withdraw';
    fast: boolean;
    validFrom: number;
    validUntil: number;
};

export type DepositOp = {
    account_id: number;
    priority_op: {
        amount: string;
        from: string;
        to: string;
        token: number;
    };
    type: 'Deposit';
};

export type FullExitOp = {
    type: 'FullExit';
    serial_id: number | null;
    priority_op: {
        token: number;
        account_id: number;
        eth_address: string;
    };
    content_hash: string | null;
    creator_address: string | null;
    withdraw_amount: string | null;
    creator_account_id: number | null;
};

export type ForcedExitOp = {
    initiatorAccountId: number;
    target: string;
    token: number;
    fee: string;
    nonce: number;
    signature: {
        pubKey: string;
        signature: string;
    };
    type: 'ForcedExit';
    validFrom: number;
    validUntil: number;
};

export type MintNFTOp = {
    fee: string;
    creatorId: number;
    nonce: number;
    signature: {
        pubKey: string;
        signature: string;
    };
    creatorAddress: string;
    recipient: string;
    contentHash: string;
    feeToken: number;
    type: 'MintNFT';
};

export type WithdrawNFTOp = {
    fee: string;
    from: string;
    accountId: number;
    nonce: number;
    signature: {
        pubKey: string;
        signature: string;
    };
    to: string;
    token: number;
    feeToken: number;
    type: 'WithdrawNFT';
    fast: boolean;
    validFrom: number;
    validUntil: number;
};

export type Order = {
    accountId: number;
    recipient: string;
    nonce: number;
    tokenBuy: number;
    tokenSell: number;
    ratio: [string, string];
    amount: string;
    validFrom: number;
    validUntil: number;
    signature: {
        pubKey: string;
        signature: string;
    };
};

export type SwapOp = {
    submitterId: number;
    submitterAddress: string;
    nonce: number;
    orders: [Order, Order];
    amounts: [string, string];
    fee: string;
    feeToken: number;
    signature: {
        pubKey: string;
        signature: string;
    };
    type: 'Swap';
};

type PriorityOpInterface<T> = {
    tx_type: string;
    from: string;
    to: string;
    token: number;
    amount: string;
    fee: null;
    block_number: number;
    nonce: number;
    created_at: string;
    fail_reason: null;
    tx: T;
    batch_id: null;
};

type L2TxInterface<T> = {
    tx_type: string;
    from: string;
    to: string;
    token: number;
    amount: string;
    fee: string;
    block_number: number;
    nonce: number;
    created_at: string;
    fail_reason: string | null;
    tx: T;
    batch_id: number | null;
};

export type Interface =
    | PriorityOpInterface<DepositOp>
    | PriorityOpInterface<FullExitOp>
    | L2TxInterface<ChangePubKeyOp>
    | L2TxInterface<TransferOp>
    | L2TxInterface<WithdrawOp>
    | L2TxInterface<ForcedExitOp>
    | L2TxInterface<MintNFTOp>
    | L2TxInterface<WithdrawNFTOp>
    | L2TxInterface<SwapOp>;
