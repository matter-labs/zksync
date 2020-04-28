type Balances = {
    '0'?: string,
    '1'?: string,
};

type Interface = {
    "id": number,
    "commited": {
        "pub_key_hash": string,
        "address": string,
        "balances": Balances,
        "nonce": number
    },
    "verified": {
        "pub_key_hash": string,
        "address": string,
        "balances": Balances,
        "nonce": number
    }
}
