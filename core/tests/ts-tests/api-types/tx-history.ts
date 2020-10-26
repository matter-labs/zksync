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
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
};

type FullExit = {
    tx_id: string;
    hash: string;
    eth_block: number;
    pq_id: number;
    tx: {
        priority_op: {
            token: string;
            account_id: number;
            eth_address: string;
        };
        withdraw_amount: string;
        type: 'FullExit';
    };
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
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
    };
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
};

type ChangePubKey = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: {
        account: string;
        accountId: number;
        ethSignature: string | null;
        newPkHash: string;
        nonce: number;
        type: string;
        feeToken: number;
        fee: string;
        signature: {
            pubKey: string;
            signature: string;
        };
    };
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
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
    };
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
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
    };
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
};

export type Interface = (Deposit | Transfer | Withdraw | ChangePubKey | FullExit | ForcedExit)[];
