const ethers = require('ethers')
const Franklin = require('../franklin/src/franklin')

const provider = new ethers.providers.JsonRpcProvider()
const franklin = new Franklin(process.env.API_SERVER, provider, process.env.CONTRACT_ADDR)
const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms))

let source = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/0").connect(provider)
let sourceNonce = null

const MIN_AMOUNT = ethers.utils.parseEther('0.1') // ~USD 15
const WITH_MARGIN = MIN_AMOUNT.add(ethers.utils.parseEther('0.04')) // ~USD 6 more for gas

var args = process.argv.slice(2)
let nClients = args[0] || 3
let tps = args[1] || 1

let clients = []

function randomClient() {
    return clients[ Math.floor(Math.random() * nClients) ]
}

console.log(`Usage: yarn test -- [nClients] [TPS]`)
console.log(`Starting loadtest for ${nClients} with ${tps} TPS`)

class Client {

    constructor(id) {
        this.id = id
        console.log(`creating client #${this.id}`)
    }

    async prepare() {
        let signer = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/" + this.id + 17)
        this.fra = await franklin.Wallet.fromSigner(signer)
        this.eth = this.fra.ethWallet
        console.log(`${this.eth.address}: prepare`)
        
        try {
            let fundingRequired = false
            await this.fra.pullState()
            if (this.fra.sidechainOpen) {
                let balance = this.fra.currentBalance
                console.log(`${this.eth.address}: sidechain account ${this.fra.sidechainAccountId}, current balance ${ethers.utils.formatEther(balance)}`)
                fundingRequired = balance.lt(MIN_AMOUNT)
            } else {
                console.log(`${this.eth.address}: sidechain account not open, deposit required`)
                fundingRequired = true
            }

            if (fundingRequired) {
                console.log(`${this.eth.address}: Franklin funding required`)

                // is wallet balance enough?
                let balance = await this.eth.getBalance()
                console.log(`${this.eth.address}: eth wallet balance is ${ethers.utils.formatEther(balance)} ETH`)
                if (balance.lt(WITH_MARGIN)) {
                    console.log(`${this.eth.address}: wallet funding required`)
                    // transfer funds from source account
                    let request = await source.sendTransaction({
                        to:     this.eth.address,
                        value:  WITH_MARGIN,
                        nonce:  sourceNonce++,
                    })
                    console.log(`${this.eth.address}: funding tx sent`)
                    let receipt = await request.wait()
                    console.log(`${this.eth.address}: funding tx mined`)
                }

                // deposit funds into franklin
                console.log(`${this.eth.address}: depositing ${ethers.utils.formatEther(MIN_AMOUNT)} ETH into Franklin`)
                let request = await this.fra.deposit(MIN_AMOUNT)
                console.log(`${this.eth.address}: deposit tx sent`)
                let receipt = await request.wait()
                console.log(`${this.eth.address}: deposit tx mined, waiting for zk proof`)
                while (!this.fra.sidechainOpen || this.fra.currentBalance.lt(MIN_AMOUNT)) {
                    await sleep(500)
                    await this.fra.pullState()
                }
                console.log(`${this.eth.address}: sidechain deposit complete`)
            }
        } catch (err) {
            console.log(`${this.eth.address}: ERROR: ${err}`)
            console.trace(err.stack)
        }
    }

    async randomTransfer() {
        let toAccountId = null
        while (true) {
            let to = randomClient()
            if (to.fra.sidechainOpen && to.fra.sidechainAccountId !== this.fra.sidechainAccountId) {
                toAccountId = to.fra.sidechainAccountId
                break
            }
        }
        console.log(`${this.eth.address}: transfer to ${toAccountId}`)
        let amount = franklin.truncate(this.fra.currentBalance.div(10))
        console.log(`${this.eth.address}: transfer(${toAccountId}, ${amount})`)
        await this.fra.transfer(toAccountId, amount)
        console.log(`${this.eth.address}: transfer done`)
    }
}

async function test() {

    sourceNonce = await source.getTransactionCount("pending")

    console.log('creating clients...')
    for (let i=0; i < nClients; i++) {
        clients.push(new Client(i))
    }

    console.log('preparing clients...')
    let promises = []
    for (let i=0; i < nClients; i++) {
        promises.push( clients[i].prepare() )
    }

    console.log('waiting until the clients are ready...')
    await Promise.all(promises)

    console.log('starting the test...')
    while(true) {
        var nextTick = new Date(new Date().getTime() + 1000);
        for (let i=0; i<tps; i++) {
            randomClient().randomTransfer()
        }
        console.log('-')
        break
        while(nextTick > new Date()) {
            await new Promise(resolve => setTimeout(resolve, 1))
        }
    }
}

test()
