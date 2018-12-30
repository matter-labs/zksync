const store = {
    contractAddress: null,
    account: {
        // ethereum part
        address:    null,
        balance:    null,
        onchain:    {
            balance:    null,
        },
        plasma: {
            id:         null,
            closing:    false,
            key:        null,
            pending_nonce: null,
            pending: {
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