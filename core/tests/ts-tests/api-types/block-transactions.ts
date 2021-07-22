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

type WithdrawNFT = {
    tx_hash: string;
    block_number: number;
    op: {
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
    created_at: string;
};

type MintNFT = {
    tx_hash: string;
    block_number: number;
    op: {
        fee: string;
        creatorId: number;
        creatorAddress: string;
        nonce: number;
        signature: {
            pubKey: string;
            signature: string;
        };
        recipient: string;
        contentHash: string;
        feeToken: number;
        type: 'MintNFT';
    };
    success: boolean;
    fail_reason: string | null;
    created_at: string;
};

export type Interface = (
    | Deposit
    | Transfer
    | Withdraw
    | ChangePubKey
    | FullExit
    | ForcedExit
    | WithdrawNFT
    | MintNFT
)[];
