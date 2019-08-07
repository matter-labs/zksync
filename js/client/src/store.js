const store = {
    config:          null,
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
        coin_balances: {
            'ETH': '10000.01',
            'BTC': '10321',
            'ZEC': '123'
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