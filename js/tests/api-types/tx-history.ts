type Deposit = {
    "hash": string,
    "pq_id": number,
    "tx": {
        "account_id": number,
        "priority_op": {
            "amount": string,
            "from": string,
            "to": string,
            "token": string,
        },
        "type": "Deposit"
    },
    "success": null,
    "fail_reason": null,
    "commited": boolean,
    "verified": boolean,
    "created_at": string,
};

type Transfer = {
    "hash": string,
    "pq_id": null,
    "tx": {
        "amount": string,
        "fee": string,
        "from": string,
        "nonce": number,
        "signature": {
            "pubKey": string,
            "signature": string
        },
        "to": string,
        "token": string,
        "type": "Transfer"
    },
    "success": boolean,
    "fail_reason"?: string,
    "commited": boolean,
    "verified": boolean,
    "created_at": string,
};

type ChangePubKey = {
    "hash": string,
    "pq_id": null,
    "tx": {
        "account": string,
        "ethSignature": null,
        "newPkHash": string,
        "nonce": number,
        "type": string,
    },
    "success": boolean,
    "fail_reason"?: string,
    "commited": boolean,
    "verified": boolean
}

export type Interface = (Deposit | Transfer | ChangePubKey)[];
