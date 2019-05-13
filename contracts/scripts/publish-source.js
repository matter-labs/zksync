const axios = require('axios')
const ethers = require('ethers')
const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL)

async function main() {

    console.log('provider:', process.env.WEB3_URL)
    console.log('ETHERSCAN_API_KEY:', process.env.ETHERSCAN_API_KEY)
    

}

main()