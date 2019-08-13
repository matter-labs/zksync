const store = {
    config:          null,
    contractAddress: null,

    onchain:    {
        address: null,
        nonce:   null,
        committed: {
            balances: {}
        },
        pending: {
            balances: {}
        },
    },
    contract: {
        committed: {
            lockedUnlockedBalances: {
                
            }
        }, 
        pending: {
            lockedUnlockedBalances: {
                
            }
        }
    },
    plasma: {
        address:    null,
        pending: {
            balances: {},
            nonce:   0
        },
        committed: {
            balance: {},
            nonce:   0
        },
        verified: {
            balance: {},
            nonce:   0
        }
    },
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