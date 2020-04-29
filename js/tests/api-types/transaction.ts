
type ChangePubKey = {
    "tx_type": "ChangePubKey",
    "from": string,
    "to": string,
    "token": number,
    "amount": string,
    "fee": string,
    "block_number": number,
    "nonce": number,
    "created_at": string,
    "fail_reason"?: string,
    "tx": {
        "account": string,
        "ethSignature"?: string,
        "newPkHash": string,
        "nonce": number,
        "type": "ChangePubKey"
    }
}

type Transfer = {
    "tx_type": "Transfer",
    "from": string,
    "to": string,
    "token": number,
    "amount": string,
    "fee": string,
    "block_number": number,
    "nonce": number,
    "created_at": string,
    "fail_reason"?: string,
    "tx": {
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
    }
}

type Withdraw = {
    "tx_type": "Withdraw",
    "from": string,
    "to": string,
    "token": number,
    "amount": string,
    "fee": string,
    "block_number": number,
    "nonce": number,
    "created_at": string,
    "fail_reason"?: string,
    "tx": {
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
    }
}

type Deposit = {
    "tx_type": "Deposit",
    "from": string,
    "to": string,
    "token": number,
    "amount": string,
    "fee": string,
    "block_number": number,
    "nonce": number,
    "created_at": string,
    "fail_reason": null,
    "tx": {
        "account_id": number,
        "priority_op": {
            "amount": string,
            "from": string,
            "to": string,
            "token": number
        },
        "type": "Deposit"
    }
}

type FullExit = {
    "tx_type": "FullExit",
    "from": string,
    "to": string,
    "token": number,
    "amount": string,
    "fee": string,
    "block_number": number,
    "nonce": number,
    "created_at": string,
    "fail_reason": null,
    "tx": {
        "priority_op": {
            "account_id": number,
            "eth_address": string,
            "token": number
        },
        "type": "FullExit",
        "withdraw_amount": string,
    }
}

export type Interface = (ChangePubKey | Transfer | Withdraw | Deposit | FullExit);
