async function setup() {

    let Web3 = require('web3')
    let web3 = new Web3(new Web3.providers.HttpProvider("http://localhost:8545"))
    let eth = web3.eth
    let personal = web3.eth.personal

    let pk = process.env.PRIVATE_KEY || '12B7678FF12FE8574AB74FFD23B5B0980B64D84345F9D637C2096CA0EF587806'
    await personal.importRawKey(pk, '')

    let accounts = await personal.getAccounts()

    let tx = {from: accounts[0], to: accounts[1], value: web3.utils.toWei("100", "ether")}
    personal.sendTransaction(tx, '')

    console.log('created accounts[1] = ', accounts[1])
}

setup()