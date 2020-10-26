type Transfer = {
    tx_hash: string;
    block_number: number;
    op: {
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
    };
    success: boolean;
    fail_reason: string | null;
    created_at: string;
};

type Deposit = {
    tx_hash: string;
    block_number: number;
    op: {
        account_id: number;
        priority_op: {
            amount: string;
            from: string;
            to: string;
            token: number;
        };
        type: 'Deposit';
    };
    success: boolean;
    fail_reason: string | null;
    created_at: string;
};

type ChangePubKey = {
    tx_hash: string;
    block_number: number;
    op: {
        account: string;
        accountId: number;
        ethSignature: string | null;
        newPkHash: string;
        feeToken: number;
        fee: string;
        nonce: number;
        signature: {
            pubKey: string;
            signature: string;
        };
        type: 'ChangePubKey';
    };
    success: boolean;
    fail_reason: string | null;
    created_at: string;
};

type Withdraw = {
    tx_hash: string;
    block_number: number;
    op: {
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
    };
    success: boolean;
    fail_reason: string | null;
    created_at: string;
};

type FullExit = {
    tx_hash: string;
    block_number: number;
    op: {
        priority_op: {
            account_id: number;
            eth_address: string;
            token: number;
        };
        type: 'FullExit';
        withdraw_amount: string | null;
    };
    success: boolean;
    fail_reason: string | null;
    created_at: string;
};

type ForcedExit = {
    tx_hash: string;
    block_number: number;
    op: {
        type: 'ForcedExit';
        initiatorAccountId: number;
        target: string;
        token: number;
        fee: string;
        nonce: number;
        signature: {
            pubKey: string;
            signature: string;
        };
    };
    success: boolean;
    fail_reason: string | null;
    created_at: string;
};

export type Interface = (Deposit | Transfer | Withdraw | ChangePubKey | FullExit | ForcedExit)[];
