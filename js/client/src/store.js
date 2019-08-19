const store = {
    config:          null,
    contractAddress: null,
    account: {
        // ethereum part
        address:    null,
        balance: null,
        ethBalances: null,
        contractBalances: null,
        plasma: {
            id:         null,
            closing:    false,
            address:        null,
            tx_pending: false,
            committed: {
                balances: null,
                nonce:   0
            },
            verified: {
                balances: null,
                nonce:   0
            }
        }
    }
}

export default store