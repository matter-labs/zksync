interface ChangePubKeyOnchain {
    type: 'Onchain';
}

interface ChangePubKeyECDSA {
    type: 'ECDSA';
    ethSignature: string;
    batchHash?: string;
}

interface ChangePubKeyCREATE2 {
    type: 'CREATE2';
    creatorAddress: string;
    saltArg: string;
    codeHash: string;
}

type ChangePubKey = {
    tx_type: 'ChangePubKey';
    from: string;
    to: string;
    token: number;
    amount: string;
    fee: string;
    block_number: number;
    nonce: number;
    created_at: string;
    fail_reason: string | null;
    tx: {
        fee?: string;
        feeToken?: number;
        account: string;
        accountId: number;
        signature: {
            pubKey: string;
            signature: string;
        };
        ethAuthData: ChangePubKeyOnchain | ChangePubKeyECDSA | ChangePubKeyCREATE2;
        newPkHash: string;
        nonce: number;
        type: 'ChangePubKey';
        ethSignature: null;
        validFrom: number;
        validUntil: number;
    };
};

type Transfer = {
    tx_type: 'Transfer';
    from: string;
    to: string;
    token: number;
    amount: string;
    fee: string;
    block_number: number;
    nonce: number;
    created_at: string;
    fail_reason: string | null;
    tx: {
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
};

type Withdraw = {
    tx_type: 'Withdraw';
    from: string;
    to: string;
    token: number;
    amount: string;
    fee: string;
    block_number: number;
    nonce: number;
    created_at: string;
    fail_reason: string | null;
    tx: {
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
};

type Deposit = {
    tx_type: 'Deposit';
    from: string;
    to: string;
    token: number;
    amount: string;
    fee: null;
    block_number: number;
    nonce: number;
    created_at: string;
    fail_reason: null;
    tx: {
        account_id: number;
        priority_op: {
            amount: string;
            from: string;
            to: string;
            token: number;
        };
        type: 'Deposit';
    };
};

type FullExit = {
    tx_type: 'FullExit';
    from: string;
    to: string;
    token: number;
    amount: string;
    fee: null;
    block_number: number;
    nonce: number;
    created_at: string;
    fail_reason: null;
    tx: {
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
};

type ForcedExit = {
    tx_type: 'ForcedExit';
    from: string;
    to: string;
    token: number;
    amount: string;
    fee: string;
    block_number: number;
    nonce: number;
    created_at: string;
    fail_reason: string | null;
    tx: {
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
};

type MintNFT = {
    tx_type: 'MintNFT';
    from: string;
    to: string;
    token: number;
    amount: string;
    fee: string;
    block_number: number;
    nonce: number;
    created_at: string;
    fail_reason: string | null;
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
};

type WithdrawNFT = {
    tx_type: 'WithdrawNFT';
    from: string;
    to: string;
    token: number;
    amount: string;
    fee: string;
    block_number: number;
    nonce: number;
    created_at: string;
    fail_reason: string | null;
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
};

export type Interface = ChangePubKey | Transfer | Withdraw | Deposit | FullExit | ForcedExit | MintNFT | WithdrawNFT;
