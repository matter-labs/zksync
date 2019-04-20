const axios = require('axios')

class Franklin {

    constructor(serverUrl) {
        this.baseUrl = serverUrl + '/api/v0.1'
        console.log(`Franklin client created for ${serverUrl}`)
    }

    parseStateResult(data) {
        if (data.error !== undefined && data.error == "non-existent") {
            data.closing = true
        } else {
            data.closing = false
        }
        const multiplier = new BN('1000000000000')
        data.verified.balance = Eth.fromWei((new BN(data.verified.balance)).mul(multiplier), 'ether')
        data.committed.balance = Eth.fromWei((new BN(data.committed.balance)).mul(multiplier), 'ether')
        data.pending.balance = Eth.fromWei((new BN(data.pending.balance)).mul(multiplier), 'ether')
        // TODO: remove when server updated
        if (Number(data.pending_nonce) > Number(data.pending.nonce)) {
            data.pending.nonce = data.pending_nonce
        }
        return data
    }

    async getPlasmaInfo(accountId) {
        //console.log(`getAccountInfo ${accountId}`)
        let result = (await axios({
            method: 'get',
            url:    this.baseUrl + '/account/' + accountId,
        }))
        if(result.status !== 200) {
            throw `Could not load data for account ${accountId}: ${result.error}`
        }
        if(result.data.error === 'non-existing account') {
            return { closing: true }
        }
        if(result.data.error) {
            throw `Getting data for account ${accountId} failed: ${result.data.error}`
        }
        return this.parseStateResult(result.data)
    }

    async getAccount(id) {
        return this.getPlasmaInfo(id)
    }

}

module.exports = Franklin