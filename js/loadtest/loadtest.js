const axios = require('axios')
const ethers = require('ethers')
const Franklin = require('../franklin/src/franklin')
var Prando = require('prando')

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL)
const franklin = new Franklin(process.env.API_SERVER, provider, process.env.CONTRACT_ADDR)
const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms))

const bn = ethers.utils.bigNumberify
const format = ethers.utils.formatEther

// taking the second account from the mnemonic
let source = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider)

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

let TOTAL_TX = 256*120
let total = 0

function randomClient() {
    let i = rng.nextInt(0, nClients-1)
    //console.log('i', i)
    return clients[ i ]
}

const withTimeout = function(ms, promise){

    // Create a promise that rejects in <ms> milliseconds
    let timeout = new Promise((resolve, reject) => {
      let id = setTimeout(() => {
        clearTimeout(id);
        reject('Timed out in '+ ms + 'ms.')
      }, ms)
    })
  
    // Returns a race between our timeout and the passed in promise
    return Promise.race([
      promise,
      timeout
    ])
  }

class Client {

    constructor(id) {
        this.id = id
        console.log(`creating client #${this.id}`)
    }

    async prepare(fundFranklin) {
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

            if ( fundFranklin && toAddFranklin.gt(0) ) {
                console.log(`${this.eth.address}: adding ${format(toAddFranklin)} to Franklin`)

                // is wallet balance enough?
                let balance = await this.eth.getBalance()
                console.log(`${this.eth.address}: eth wallet balance is ${format(balance)} ETH`)
                let minBalance = MIN_AMOUNT_FRA.add(transferPrice.mul(2))
                if (balance.lt(minBalance)) {
                    let toAdd = bn("80000000000000000") //minBalance.sub(balance)
                    console.log(`${this.eth.address}: adding ${format(toAdd)} to eth wallet`)
                    // transfer funds from source account
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
            } else {
                console.log('${this.eth.address}: prepared')
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

    async performExit() {
        try {
            let request = await this.fra.exit()
            console.log(`${this.eth.address}: full exit tx sent`)
            let receipt = await request.wait()
        } catch (err) {
            console.log(`${this.eth.address}: EXIT ERROR: ${err}`)
            console.trace(err.stack)
            throw err
        }
    }
}

async function waitForVerifyBlocksPromise() {
    while(true){
        try {
            let response = await axios({
                method:     'get',
                url:        clients[0].fra.fra.baseUrl + '/blocks',
            });
            let blocks = response.data
            let unverified_found = false
            for (let i=0; i < blocks.length; i++) {
                if (blocks[i].verify_tx_hash == null) {
                    unverified_found = true
                    break
                }
            }
            if (!unverified_found) {
                break
            }
            await sleep(1500)
        } catch (err) {
            console.log(`Get blocks request error: ${err}`)
            continue
        }
    }
}

async function test() {

    var args = process.argv.slice(2);
    let prepareOnly = args[0] === 'prepare'

    let fundFranklin = !prepareOnly

    console.log('Will run:', fundFranklin)

    let sourceBalanceBefore = await source.getBalance()
    sourceNonce = await source.getTransactionCount("pending")
    gasPrice = (await provider.getGasPrice()).mul(2)

    console.log(`Current gas price: ${gasPrice.div(1000000000).toNumber()} GWEI`)
    transferPrice = gasPrice.mul(DEPOSIT_GAS_LIMIT)

    console.log('creating clients...')
    for (let i=0; i < nClients; i++) {
        clients.push(new Client(i))
    }

    console.log('xx: preparing clients...')
    let promises = []
    for (let i=0; i < nClients; i++) {
        promises.push( clients[i].prepare(fundFranklin) )
    }

    //console.log('waiting until the clients are ready...')
    await Promise.all(promises)

    if (prepareOnly) process.exit(0);

    let sourceBalanceAfter = await source.getBalance()
    console.log('Total spent: ', format(sourceBalanceBefore.sub(sourceBalanceAfter)))

    console.log('starting the transfers test...')
    while(total < TOTAL_TX) {

        let promises = []
        for (let i=0; i < nClients; i++) {
            promises.push(clients[i].fra.pullState().catch(e => 'err3: ' + e))
        }
        await withTimeout(1500, Promise.all(promises)).catch(e => 'err4: ' + e)

        promises = []
        for (let i=0; i<(tps * 3); i++) {
            let client = randomClient()
            let promise = client.randomTransfer().catch(e => console.log('err1: ', e))
            promises.push(promise)
            total++
        }
        await withTimeout(1500, Promise.all(promises)).catch(e => 'err2: ' + e)

        console.log('-- total: ', total, ' of ', TOTAL_TX)
    }

    console.log('transfers test complete, total = ', total)

    console.log('waiting for all blocks verification')
    let promise = waitForVerifyBlocksPromise()
    await promise.catch(e => 'err7: ' + e)

    console.log('performing exits from clients...')

    promises = []
    for (let i=0; i < nClients; i++) {
        promises.push( clients[i].performExit().catch(e => 'err5: ' + e) )
    }
    await withTimeout(1500, Promise.all(promises)).catch(e => 'err6: ' + e)

    console.log('waiting for all blocks verification')
    await promise.catch(e => 'err7: ' + e)
    
    console.log('loadtest finished')
}

test()
