export function compareObjectStructure(a, b) {
    function go(path, obj) {
        for (const key of path) {
            obj = obj[key];
        }
        return obj;
    }

    const type = o => Array.isArray(o) ? 'array' : typeof o;

    function compare(path) {
        const objA = go(path, a);
        const objB = go(path, b);

        if (type(objA) != type(objB)) {
            throw path;
        }

        if (type(objA) == 'array' && objA.length > 0) {
            compare([...path, 0]);
        } else if (objA != null && typeof objA == 'object') {
            for (const key of Object.keys(objA)) {
                compare([...path, key]);
            }
        }
    }

    try {
        compare([]);
        return true;
    } catch (e) {
        console.log(e);
        return false;
    }
}

export const transactionHistoryItems = [
    {
        "hash": "0x081f802afbe7c5f3840bc90f2d3776a5a9b86db9535431870b57c6b45f0b6494",
        "pq_id": 20,
        "tx": {
            "account_id": 21,
            "priority_op": {
                "amount": "9000000000000000",
                "from": "0x84764887da7aa7688f88118328fca2a635cdb502",
                "to": "0x43aac75f8d78f3f4bb40cbe776ff008dce453a7c",
                "token": "ERC20-1"
            },
            "type": "Deposit"
        },
        "success": null,
        "fail_reason": null,
        "commited": true,
        "verified": false
    },
    {
        "hash": "sync-tx:2ff7bae1e3c2915e770418d74d2053818f3377c9d5328e430d4cd0c03df894cb",
        "pq_id": null,
        "tx": {
            "amount": "3000000000000000",
            "fee": "30000000000000",
            "from": "0x43aac75f8d78f3f4bb40cbe776ff008dce453a7c",
            "nonce": 3,
            "signature": {
                "pubKey": "b9b2327cae93ee4cc4bdc1c7c6537ab85318d58b16ba887eba7a652d7f21d00c",
                "signature": "898d4680be0dfbbf2ee7a554a86a30c21dd32ddfc656bb16502e2369b8729322aa7054e9c635b7209ebb3e77f396fb8a148cd1bf7c1ceeefb58a95e3db452a05"
            },
            "to": "0x43aac75f8d78f3f4bb40cbe776ff008dce453a7c",
            "token": "ERC20-1",
            "type": "Transfer"
        },
        "success": true,
        "fail_reason": null,
        "commited": true,
        "verified": false
    },
    {
        "hash": "sync-tx:11b81763ca5686199c1383e576039fa8cdc6fcfab2fb009d9bfeb4bf6b223ff8",
        "pq_id": null,
        "tx": {
            "account": "0x43aac75f8d78f3f4bb40cbe776ff008dce453a7c",
            "ethSignature": null,
            "newPkHash": "sync:5812bc0b57dae5a2b2139eec9b02549f86596c5f",
            "nonce": 0,
            "type": "ChangePubKey"
        },
        "success": true,
        "fail_reason": null,
        "commited": true,
        "verified": false
    }
];
