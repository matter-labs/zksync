const store = {
    contractAddress: null,
    account: {
        // ethereum part
        address:    null,
        balance:    null,
        onchain:    {
            //isClosing: false,
            balance:    null,
            completeWithdrawArgs: null,
        },
        plasma: {
            id:         null,
            closing:    false,
            key:        null,
            pending_nonce: 0,
            pending: {
                nonce:   0
            },
            committed: {
                balance: null,
                nonce:   0
            },
            verified: {
                balance: null,
                nonce:   0
            }
        }
    }
}

export default store