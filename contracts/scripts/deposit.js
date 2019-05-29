const ethers = require('ethers')
const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL)

const PlasmaContractABI = require('../build/contracts/FranklinProxy.json').abi
//const PlasmaContractABI = JSON.parse(fs.readFileSync('./contracts/build/contracts/FranklinProxy.json', 'utf8')).abi

const source = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/0").connect(provider)
const franklin = new ethers.Contract(process.env.CONTRACT_ADDR, PlasmaContractABI, source)

async function main() {
    let paddingPubKey = JSON.parse(process.env.PADDING_PUB_KEY)
    let value = ethers.utils.parseEther("0.001")
    let r = await franklin.deposit(paddingPubKey, 0, {value})
    console.log(r)
}

main()
