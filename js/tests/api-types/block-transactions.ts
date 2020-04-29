type Transfer = {
    "tx_hash": string,
    "block_number": number,
    "op": {
        "amount": string,
        "fee": string,
        "from": string,
        "nonce": number,
        "signature": {
            "pubKey": string,
            "signature": string,
        },
        "to": string,
        "token": number,
        "type": "Transfer"
    },
    "created_at": string,
};

type Deposit = {
    "tx_hash": string,
    "block_number": number,
    "op": {
        "account_id": number,
        "priority_op": {
            "amount": string,
            "from": string,
            "to": string,
            "token": number
        },
        "type": "Deposit"
    },
    "created_at": string,
};

type ChangePubKey = {
    "tx_hash": string,
    "block_number": number,
    "op": {
        "account": string,
        "ethSignature"?: string,
        "newPkHash": string,
        "nonce": number,
        "type": "ChangePubKey"
    },
    "created_at": string,
};

type Withdraw = {
    "tx_hash": string,
    "block_number": number,
    "op": {
        "amount": string,
        "fee": string,
        "from": string,
        "nonce": number,
        "signature": {
            "pubKey": string,
            "signature": string,
        },
        "to": string,
        "token": number,
        "type": "Withdraw"
    },
    "created_at": string,
};

type FullExit = {
    "tx_hash": string,
    "block_number": number,
    "op": {
        "priority_op": {
            "account_id": number,
            "eth_address": string,
            "token": number
        },
        "type": "FullExit",
        "withdraw_amount": string
    },
    "created_at": string,
};

export type Interface = (Deposit | Transfer | Withdraw | ChangePubKey | FullExit)[];
