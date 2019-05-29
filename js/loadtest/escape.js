const ethers = require('ethers')
const Franklin = require('../franklin/src/franklin')
var Prando = require('prando')

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL)
const franklin = new Franklin(process.env.API_SERVER, provider, process.env.CONTRACT_ADDR)
const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms))

const bn = ethers.utils.bigNumberify
const format = ethers.utils.formatEther

// taking the second account from the mnemonic
let source = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/0").connect(provider)

const DEPOSIT_GAS_LIMIT = bn(100000)
const MIN_AMOUNT_FRA = ethers.utils.parseEther(process.env.LOADTEST_MIN_AMOUNT)

// to populate from promises
let sourceNonce = null
let gasPrice = null
let transferPrice = null

let nClients = process.env.LOADTEST_N_CLIENTS
let tps = process.env.LOADTEST_TPS

let clients = []

let rng = new Prando(1) // deterministic seed

let TOTAL_TX = 256*250
let total = 0

class Client {

    constructor(id) {
        this.id = id
        console.log(`creating client #${this.id}`)
    }

    async prepare() {
        let signer = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/3/" + this.id)
        this.fra = await franklin.Wallet.fromSigner(signer)
        this.eth = this.fra.ethWallet
        console.log(`${this.eth.address}: prepare`)
        
        try {
            let toAddFranklin = bn(0)
            await this.fra.pullState()
            if (this.fra.sidechainOpen) {
                let balance = this.fra.currentBalance
                console.log(`${this.eth.address}: sidechain account ${this.fra.sidechainAccountId}, `,
                    `current balance ${format(balance)}`)
                if (balance.lt(MIN_AMOUNT_FRA)) toAddFranklin = MIN_AMOUNT_FRA.sub(balance)
            } else {
                console.log(`${this.eth.address}: sidechain account not open, deposit required`)
                toAddFranklin = MIN_AMOUNT_FRA
            }

            if ( toAddFranklin.gt(0) ) {
                console.log(`${this.eth.address}: adding ${format(toAddFranklin)} to Franklin`)

                // is wallet balance enough?
                let balance = await this.eth.getBalance()
                console.log(`${this.eth.address}: eth wallet balance is ${format(balance)} ETH`)
                let minBalance = MIN_AMOUNT_FRA.add(transferPrice.mul(2))
                if (balance.lt(minBalance)) {
                    let toAdd = minBalance.sub(balance)
                    console.log(`${this.eth.address}: adding ${format(toAdd)} to eth wallet`)
                    // transfer funds from source account
                    let signer = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/3/" + this.id)
                    let request = await source.sendTransaction({
                        to:     this.eth.address,
                        value:  toAdd,
                        nonce:  sourceNonce++,
                    })
                    console.log(`${this.eth.address}: funding tx sent`)
                    let receipt = await request.wait()
                    console.log(`${this.eth.address}: funding tx mined`)
                }

                // deposit funds into franklin
                console.log(`${this.eth.address}: depositing ${format(MIN_AMOUNT_FRA)} ETH into Franklin`)
                let request = await this.fra.deposit(MIN_AMOUNT_FRA)
                console.log(`${this.eth.address}: deposit tx sent`)
                let receipt = await request.wait()
                console.log(`${this.eth.address}: deposit tx mined, waiting for zk proof`)
                while (!this.fra.sidechainOpen || this.fra.currentBalance.lt(MIN_AMOUNT_FRA)) {
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
        let fromAccountId = this.fra.sidechainAccountId
        let toAccountId = null
        while (true) {
            let to = randomClient()
            //console.log(to)
            if (to.fra.sidechainOpen && to.fra.sidechainAccountId !== fromAccountId) {
                toAccountId = to.fra.sidechainAccountId
                break
            }
        }
        let balance_int = this.fra.currentBalance.div('1000000000000').div(10).toNumber()
        if (balance_int < 12) {
            console.log('skip tx')
            return
        }

        let round_amount = rng.nextInt(11, balance_int - 1)
        let amount = 
            ethers.utils.bigNumberify(round_amount)
            //ethers.utils.bigNumberify(20474)
            .mul('1000000000000')

        //console.log(`${this.eth.address}: transfer ${round_amount} from ${fromAccountId} to ${toAccountId}...`);

        let transferData = `transfer ${round_amount} from ${fromAccountId} to ${toAccountId} nonce ${this.fra.nextNonce}`;
        //console.log(`${this.eth.address}: ${transferData} requesting...`)
        let r = await this.fra.transfer(toAccountId, amount)
        if (r.accepted) {
            console.log(`${this.eth.address}: ${transferData} ok`)
        } else {
            console.log(`${this.eth.address}: ${transferData} failed: ${JSON.stringify(r)}`)
        }
    }
}

async function test() {

    let sourceBalanceBefore = await source.getBalance()
    sourceNonce = await source.getTransactionCount("pending")
    gasPrice = (await provider.getGasPrice()).mul(11)

    for (let i = 166; i < 200; i++) {
    let r = await source.sendTransaction({
        to:     '0x472F371dcEB55cC845fAebF03F90f2DfEc603185',
        value:  ethers.utils.parseEther('0.029'),
        nonce:  i,
    }).catch(e => console.log(e))
    console.log(r)
}
}

test()
