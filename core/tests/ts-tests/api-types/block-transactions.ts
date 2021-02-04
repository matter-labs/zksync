interface ChangePubKeyOnchain {
    type: 'Onchain';
}

interface ChangePubKeyECDSA {
    type: 'ECDSA';
    ethSignature: string;
    batchHash: string;
}

interface ChangePubKeyCREATE2 {
    type: 'CREATE2';
    creatorAddress: string;
    saltArg: string;
    codeHash: string;
}

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
        validFrom: number;
        validUntil: number;
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
        newPkHash: string;
        feeToken: number;
        fee: string;
        nonce: number;
        ethAuthData: ChangePubKeyOnchain | ChangePubKeyECDSA | ChangePubKeyCREATE2;
        ethSignature: string | null;
        signature: {
            pubKey: string;
            signature: string;
        };
        type: 'ChangePubKey';
        validFrom: number;
        validUntil: number;
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
        validFrom: number;
        validUntil: number;
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
        validFrom: number;
        validUntil: number;
    };
    success: boolean;
    fail_reason: string | null;
    created_at: string;
};

export type Interface = (Deposit | Transfer | Withdraw | ChangePubKey | FullExit | ForcedExit)[];
