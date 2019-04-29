const axios = require('axios')
const ethers = require('ethers')
const PlasmaContractABI = require('./PlasmaContract.json').abi

class FranklinWallet {
    constructor(franklin, ethAddress, privateKey) {
        this.fra = franklin
        this.eth = franklin.eth

        this.ethAddress = ethAddress
        this.privateKey = privateKey

        this.sidechainAccountId = null

        console.log(`new FranklinWallet(${this.ethAddress}, ${this.privateKey})`)
    }

    async pullState() {
        this.sidechainAccountId = await this.eth.contract.ethereumAddressToAccountID(this.ethAddress)
    }
}

class Wallet {
    constructor(franklin) {
        this.franklin = franklin
        this.LoginMessage = 'Login Franklin v0.1'
    }

    fromEthAddress(ethAddress, privateKey) {
        return new FranklinWallet(this.franklin, ethAddress, privateKey)
    }
}

class Franklin {
    constructor(serverUrl, provider, contractAddress) {
        this.baseUrl = serverUrl + '/api/v0.1'
        this.Wallet = new Wallet(this)
        this.eth = {
            provider,
            contractAddress
        }
        
        if (typeof contractAddress !== 'string' || contractAddress.length < 4) throw 'Invalid contract address: ' + contractAddress
        if (!contractAddress.startsWith('0x')) contractAddress = '0x' + contractAddress
        console.log('contractAddress', contractAddress)
        this.eth.contract = new ethers.Contract(contractAddress, PlasmaContractABI, provider)
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