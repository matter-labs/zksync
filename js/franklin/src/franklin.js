const axios = require('axios')
const ethers = require('ethers')
const {keccak256} = require('js-sha3')
const transaction = require('./transaction.js')
const PlasmaContractABI = require('../abi/PlasmaContract.json').abi

const MULTIPLIER = ethers.utils.bigNumberify('1000000000000')

class FranklinWallet {
    constructor(franklin, ethAddress, privateKeySeed, signer) {
        this.fra = franklin
        this.eth = franklin.eth
        this.ethWallet = signer.connect(franklin.eth.provider)
        this.nextNonce = 0

        this.ethAddress = ethAddress

        if (!privateKeySeed) throw 'Cannot create FranklinWallet: privateKeySeed must be valid'
        this.key = transaction.newKey(privateKeySeed)

        console.log(`Private key for ${signer.address}: ${JSON.stringify(this.key)}`)

        this.sidechainAccountId = null
        this.sidechainState = null

        console.log(`new FranklinWallet(${this.ethAddress})`)
    }

    async pullState(checkAddress) {
        checkAddress = checkAddress || !this.sidechainAccountId
        this.sidechainAccountId = await this.eth.contract.ethereumAddressToAccountID(this.ethAddress)
        this.sidechainState = this.sidechainAccountId > 0 ?
            await this.fra.pullSidechainState(this.sidechainAccountId) : null
        if (this.sidechainState && this.sidechainState.current && this.sidechainState.current.nonce > this.nextNonce) {
            this.nextNonce = this.sidechainState.current.nonce
        }
    }

    get sidechainOpen() {
        return this.sidechainAccountId && this.sidechainState.state === 'open'
    }

    get currentBalance() {
        return this.sidechainOpen ? this.sidechainState.current.balance : undefined
    }

    async deposit(amount) {
        if (!this.ethWallet) {
            throw 'Can not initiate deposit into Franklin: no wallet connected'
        }

        // // Normally we would let the Wallet populate this for us, but we
        // // need to compute EXACTLY how much value to send
        // let gasPrice = await this.eth.provider.getGasPrice();

        // // The exact cost (in gas) to send to an Externally Owned Account (EOA)
        // let gasLimit = 110000;

        // // The balance less exactly the txfee in wei
        // let value = amount.sub(gasPrice.mul(gasLimit))

        let value = amount

        let pubX = ethers.utils.bigNumberify(this.key.publicKey.x.toString())
        let pubY = ethers.utils.bigNumberify(this.key.publicKey.y.toString())
        let maxFee = ethers.utils.parseEther('0.0')

        let contract = this.eth.contract.connect(this.ethWallet)
        return contract.deposit([pubX, pubY], maxFee, {value})
    }

    async transfer(to, amount) {
        if ( !this.sidechainOpen ) {
            throw ''
        }

        if (!this.fra.truncate(amount).eq(amount)) {
            throw 'Amount must be rounded with franklin.truncate(): ' + amount
        }

        // TODO: if `to` is address, convert it to sidechainAccountId

        const from = this.sidechainAccountId
        amount = amount.div(MULTIPLIER).toNumber()
        const privateKey = this.key.privateKey
        //console.log(this.sidechainState)
        const nonce = this.nextNonce++
        const good_until_block = 50000 // TODO: add to current block?
        const fee = 0;

        const apiForm = transaction.createTransaction(from, to, amount, fee, nonce, good_until_block, privateKey);
        const result = await axios({
            method:     'post',
            url:        this.fra.baseUrl + '/submit_tx',
            data:       apiForm
        });
        await new Promise(resolve => setTimeout(resolve, 500))
        await this.pullState(false)
        let error = result.data && result.data.error
        if (error === 'CurrentNonceIsHigher' && this.sidechainState.current.nonce < nonce) {
            this.nextNonce = this.sidechainState.current.nonce
        }
        return result.data
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
        //console.log('contractAddress', contractAddress)
        this.eth.contract = new ethers.Contract(contractAddress, PlasmaContractABI, provider)
        //console.log(`Franklin client created for ${serverUrl}`)
    }

    _parseStateResult(data) {
        if (data.error !== undefined && data.error == "non-existent") {
            data.state = 'closing'
        } if (!data.verified) {
            data.state = 'opening'
        } else {
            data.state = 'open'
            data.current = data.pending
            delete data.pending
            data.verified.balance = ethers.utils.bigNumberify(data.verified.balance).mul(MULTIPLIER)
            data.committed.balance = ethers.utils.bigNumberify(data.committed.balance).mul(MULTIPLIER)
            data.current.balance = ethers.utils.bigNumberify(data.current.balance).mul(MULTIPLIER)
        }
        return data
    }

    async pullSidechainState(accountId) {
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

    truncate(amount) {
        return ethers.utils.bigNumberify(Math.floor(amount.div(MULTIPLIER).toNumber())).mul(MULTIPLIER)
    }

}

module.exports = Franklin