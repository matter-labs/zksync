const store = {
    config:          null,
    contractAddress: null,
    supportedTokens: null,
    account: {
        // ethereum part
        address:    null,
        balance: null,
        ethBalances: null,
        contractBalances: null,
        // plasma part
        franklinAddress: null,
        commitedPlasmaBalances: null,
        verifiedPlasmaBalances: null,
    }
}

export default store