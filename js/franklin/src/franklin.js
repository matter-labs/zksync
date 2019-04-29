const axios = require('axios')
const ethers = require('ethers')
const {keccak256} = require('js-sha3')
const {newKey} = require('./transaction.js')
const PlasmaContractABI = require('../abi/PlasmaContract.json').abi

class FranklinWallet {
    constructor(franklin, ethAddress, privateKeySeed, signer) {
        this.fra = franklin
        this.eth = franklin.eth
        this.ethWallet = signer.connect(franklin.eth.provider)

        this.ethAddress = ethAddress

        if (!privateKeySeed) throw 'Cannot create FranklinWallet: privateKeySeed must be valid'
        this.key = newKey(privateKeySeed)

        this.sidechainAccountId = null
        this.sidechainState = null

        console.log(`new FranklinWallet(${this.ethAddress})`)
    }

    async pullState() {
        this.sidechainAccountId = await this.eth.contract.ethereumAddressToAccountID(this.ethAddress)
        this.sidechainState = this.sidechainAccountId > 0 ?
            await this.fra.pullSidechainState(this.sidechainAccountId) : null
    }

    get sidechainOpen() {
        return this.sidechainAccountId && true
    }

    async deposit(amount) {
        if (!this.ethWallet) {
            throw 'Can not initiate deposit into Franklin: no wallet connected'
        }

        // Normally we would let the Wallet populate this for us, but we
        // need to compute EXACTLY how much value to send
        let gasPrice = await this.eth.provider.getGasPrice();

        // The exact cost (in gas) to send to an Externally Owned Account (EOA)
        let gasLimit = 210000;

        // The balance less exactly the txfee in wei
        let value = amount.sub(gasPrice.mul(gasLimit))

        let pubX = ethers.utils.bigNumberify(this.key.publicKey.x.toString())
        let pubY = ethers.utils.bigNumberify(this.key.publicKey.y.toString())
        let maxFee = ethers.utils.parseEther('0.0')

        let contract = this.eth.contract.connect(this.ethWallet)
        return contract.deposit([pubX, pubY], maxFee, {value})
    }
    
}

class Wallet {
    constructor(franklin) {
        this.franklin = franklin
        this.LoginMessage = 'Login Franklin v0.1'
    }

    fromAddressAndSeed(ethAddress, privateKeySeed) {
        return new FranklinWallet(this.franklin, ethAddress, privateKeySeed)
    }

    async fromSigner(signer) {
        // console.log( await this.eth.signMessage( franklin.Wallet.LoginMessage ) )
        let privateKey = keccak256( await signer.signMessage( this.LoginMessage ) )
        return new FranklinWallet(this.franklin, signer.address, privateKey, signer)
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

    _parseStateResult(data) {
        if (data.error !== undefined && data.error == "non-existent") {
            data.closing = true
        } else {
            data.closing = false
        }
        const multiplier = ethers.utils.bigNumberify('1000000000000')
        data.verified.balance = ethers.utils.formatEther((ethers.utils.bigNumberify(data.verified.balance)).mul(multiplier))
        data.committed.balance = ethers.utils.formatEther((ethers.utils.bigNumberify(data.committed.balance)).mul(multiplier))
        data.pending.balance = ethers.utils.formatEther((ethers.utils.bigNumberify(data.pending.balance)).mul(multiplier))
        // TODO: remove when server updated
        if (Number(data.pending_nonce) > Number(data.pending.nonce)) {
            data.pending.nonce = data.pending_nonce
        }
        return data
    }

    async pullSidechainState(accountId) {
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
        return this._parseStateResult(result.data)
    }

}

module.exports = Franklin