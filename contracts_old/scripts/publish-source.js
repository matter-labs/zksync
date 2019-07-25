const axios = require('axios')
const ethers = require('ethers')
const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL)
const url = require('url')
const providerUrl = url.parse(process.env.WEB3_URL)
const network = providerUrl.host.split('.')[0]
const querystring = require('querystring')

const etherscanApiUrl = network === 'mainnet' ? 'https://api.etherscan.io/api' : `https://api-${network}.etherscan.io/api`

const fs = require('fs');

const FILE = 'deploy.log'
const deployLog = fs.readFileSync(FILE, 'utf8');

const ENV_FILE = process.env.ENV_FILE
const config = fs.readFileSync(ENV_FILE, 'utf8');

const ABI = JSON.parse(fs.readFileSync('./contracts/build/contracts/FranklinProxy.json', 'utf8')).abi
const Constructor = ABI.find(i => i.type === 'constructor')

function addr(name) {
    let part = deployLog
    part = part.split("Starting migrations...")[1]
    part = part.split(`'${name}'`)[1]
    part = part.split('contract address:    ')[1]
    part = part.split('\n')[0]
    return part
}

let failed = false

async function publish(name, contractaddress, constructorArguements) {

    console.log(`${name}: ${contractaddress}`)

    const file = `contracts/flat/${name}.sol`
    const sourceCode = fs.readFileSync(file, 'utf8');
    if (!sourceCode) {
        console.error(`Missing file ${file}`)
        exit(1)
    }

    let data = {
        apikey:             process.env.ETHERSCAN_API_KEY,  //A valid API-Key is required        
        module:             'contract',                     //Do not change
        action:             'verifysourcecode',             //Do not change
        contractaddress,                                    //Contract Address starts with 0x...     
        sourceCode,                                         //Contract Source Code (Flattened if necessary)
        contractname:       name,                           //ContractName
        compilerversion:    'v0.4.24+commit.e67f0147',      // see http://etherscan.io/solcversions for list of support versions
        optimizationUsed:   0,                              //0 = Optimization used, 1 = No Optimization
        runs:               200,                            //set to 200 as default unless otherwise         
        constructorArguements                               //if applicable
    }
    
    let r = await axios.post(etherscanApiUrl, querystring.stringify(data))
    if (r.data.status != 1) {
        console.log(r.data)
        failed = true
    }
}

async function main() {

    let regex = /CONTRACT_ADDR=(.*)/g
    let fromConfig = '0x'+regex.exec(config)[1]
    console.log(fromConfig)

    let FranklinProxy = addr('FranklinProxy')
    if (FranklinProxy !== fromConfig) {
        console.error(`FranklinProxy contract addresses mismatch: form env = ${fromConfig}, from ${FILE} = ${FranklinProxy}`)
        process.exit(1)
    }

    console.log(`Publishing contract sources via ${etherscanApiUrl}`)

    let Depositor = addr('Depositor')
    let Transactor = addr('Transactor')
    let Exitor = addr('Exitor')

    let promises = []

    let params = ethers.utils.defaultAbiCoder.encode(
        Constructor.inputs, 
        [Depositor, Transactor, Exitor]).substr(2)

    promises.push(publish('FranklinProxy', FranklinProxy, params))
    promises.push(publish('Depositor', Depositor))
    promises.push(publish('Transactor', Transactor))
    promises.push(publish('Exitor', Exitor))

    await Promise.all(promises)
    if (failed) process.exit(1)
}

main()
