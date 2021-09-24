import { Order } from './transaction';

type ChangePubKeyOnchain = {
    type: 'Onchain';
};

type ChangePubKeyECDSA = {
    type: 'ECDSA';
    ethSignature: string;
    batchHash?: string;
};

type ChangePubKeyCREATE2 = {
    type: 'CREATE2';
    creatorAddress: string;
    saltArg: string;
    codeHash: string;
};

type Deposit = {
    tx_id: string;
    hash: string;
    eth_block: number;
    pq_id: number;
    tx: {
        account_id: number;
        priority_op: {
            amount: string;
            from: string;
            to: string;
            token: string;
        };
        type: 'Deposit';
    };
    success: boolean;
    fail_reason: null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: null;
};

type FullExit = {
    tx_id: string;
    hash: string;
    eth_block: number;
    pq_id: number;
    tx: {
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
    success: boolean;
    fail_reason: null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: null;
};

type Transfer = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: {
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
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

type ChangePubKey = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: {
        account: string;
        accountId: number;
        newPkHash: string;
        nonce: number;
        type: string;
        feeToken: number;
        fee: string;
        ethAuthData: ChangePubKeyOnchain | ChangePubKeyECDSA | ChangePubKeyCREATE2 | null;
        ethSignature: string | null;
        signature: {
            pubKey: string;
            signature: string;
        };
        validFrom: number;
        validUntil: number;
    };
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

type Withdraw = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: {
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
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

type ForcedExit = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: {
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
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

type MintNFT = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: {
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
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

type WithdrawNFT = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: {
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
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

type Swap = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: {
        submitterId: number;
        submitterAddress: string;
        nonce: number;
        orders: Order[];
        amounts: string[];
        fee: string;
        feeToken: number;
        signature: {
            pubKey: string;
            signature: string;
        };
        type: 'Swap';
    };
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

export type Interface = (
    | Deposit
    | Transfer
    | Withdraw
    | ChangePubKey
    | FullExit
    | ForcedExit
    | MintNFT
    | WithdrawNFT
    | Swap
)[];
