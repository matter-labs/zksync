const store = {
    contractAddress: null,
    account: {
        // ethereum part
        address:    null,
        balance:    null,
        plasma: {
            id:         null,

            key:        null,

            onchain:    null,
            pending_nonce: null,
            pending: {
                balance: null,
                nonce:   null
            },
            committed: {
                balance: null,
                nonce:   null
            },
            verified: {
                balance: null,
                nonce:   null
            }
        }
    }
}

export default store