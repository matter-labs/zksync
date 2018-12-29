const store = {
    account: {
        address:    null,
        balance:    null,
        plasma: {
            id:         null,

            // verified
            balance:    null,
            key:        null,

            pending: {
                balance: null,
                nonce:   null
            }
        }
    }
}

export default store