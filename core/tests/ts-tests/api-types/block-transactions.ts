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
    batch_id: number | null;
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
    fail_reason: null;
    created_at: string;
    batch_id: null;
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
    batch_id: number | null;
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
    batch_id: number | null;
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
    fail_reason: null;
    created_at: string;
    batch_id: null;
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
    batch_id: number | null;
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
    batch_id: number | null;
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
    batch_id: number | null;
};

type Order = {
    accountId: number;
    recipient: string;
    nonce: number;
    tokenBuy: number;
    tokenSell: number;
    ratio: string[];
    amount: string;
    validFrom: number;
    validUntil: number;
    signature: {
        pubKey: string;
        signature: string;
    };
};

type Swap = {
    tx_hash: string;
    block_number: number;
    op: {
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
    | WithdrawNFT
    | MintNFT
    | Swap
)[];
