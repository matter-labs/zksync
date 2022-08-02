import { Order, ChangePubKeyOnchain, ChangePubKeyECDSA, ChangePubKeyCREATE2, ChangePubKeyEIP712 } from './transaction';

type DepositOp = {
    account_id: number;
    priority_op: {
        amount: string;
        from: string;
        to: string;
        token: string;
    };
    type: 'Deposit';
};

type FullExitOp = {
    type: 'FullExit';
    serial_id: number | null;
    priority_op: {
        token: string;
        account_id: number;
        eth_address: string;
    };
    content_hash: string | null;
    creator_address: string | null;
    withdraw_amount: string;
    creator_account_id: number | null;
};

type TransferOp = {
    accountId: number;
    amount: string;
    fee: string;
    from: string;
    nonce: number;
    signature: {
        pubKey: string;
        signature: string;
    };
    to: string;
    token: string;
    type: 'Transfer';
    validFrom: number;
    validUntil: number;
};

type ChangePubKeyOp = {
    account: string;
    accountId: number;
    newPkHash: string;
    nonce: number;
    type: string;
    feeToken: number;
    fee: string;
    ethAuthData: ChangePubKeyOnchain | ChangePubKeyECDSA | ChangePubKeyCREATE2 | ChangePubKeyEIP712 | null;
    ethSignature: string | null;
    signature: {
        pubKey: string;
        signature: string;
    };
    chainId?: number;
    validFrom: number;
    validUntil: number;
};

type WithdrawOp = {
    amount: string;
    accountId: number;
    fee: string;
    from: string;
    nonce: number;
    signature: {
        pubKey: string;
        signature: string;
    };
    to: string;
    token: string;
    type: 'Withdraw';
    fast: boolean;
    validFrom: number;
    validUntil: number;
};

type ForcedExitOp = {
    initiatorAccountId: number;
    target: string;
    token: string;
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

type MintNFTOp = {
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

type WithdrawNFTOp = {
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

type SwapOp = {
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
    tx_id: string;
    hash: string;
    eth_block: number;
    pq_id: number;
    tx: T;
    success: true;
    fail_reason: null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: null;
};

type L2TxInterface<T> = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: T;
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

export type Interface = (
    | PriorityOpInterface<DepositOp>
    | PriorityOpInterface<FullExitOp>
    | L2TxInterface<TransferOp>
    | L2TxInterface<WithdrawOp>
    | L2TxInterface<ChangePubKeyOp>
    | L2TxInterface<ForcedExitOp>
    | L2TxInterface<MintNFTOp>
    | L2TxInterface<WithdrawNFTOp>
    | L2TxInterface<SwapOp>
)[];
