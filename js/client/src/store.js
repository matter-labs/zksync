const store = {
    config:          null,
    contractAddress: null,

    onchain:    {
        address: null,
        nonce:   null,
        committed: {
            balances: {},
            balanceDict: {}
        },
        pending: {
            balances: {},
            balanceDict: {}
        },
        allTokensList: [],
        allTokensInfo: [{}],
        allTokensDict: {
            'ETH': {}
        }
    },
    contract: {
        committed: {
            lockedUnlockedBalances: {},
            balanceDict: {}
        }, 
        pending: {
            lockedUnlockedBalances: {},
            balanceDict: {}
        }
    },
    plasma: {
        address:    null,
        pending: {
            balances: {},
            balanceDict: {},
            nonce:   0
        },
        committed: {
            balance: {},
            balanceDict: {},
            nonce:   0
        },
        verified: {
            balance: {},
            balanceDict: {},
            nonce:   0
        },
        allTokensList: [],
        allTokensInfo: [],
        allTokensDict: {
            'ETH': {}
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