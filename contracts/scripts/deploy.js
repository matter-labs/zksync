const franklinContract = require('../build/Franklin');
const {
    deployContract
} = require('ethereum-waffle');
const ethers = require('ethers');
const erc20token = require('./erc20_token.js');
const ether = require('./ether.js');

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider); ////"fine music test violin matrix prize squirrel panther purchase material script deal"

async function deployFranklin(wallet, franklin) {
    try {
        let contract = await deployContract(wallet, franklin, [ethers.constants.HashZero, wallet.address, wallet.address], {
            gasLimit: 8000000
        });
        console.log("Franklin address:" + contract.address);

        return contract
    } catch (err) {
        console.log("Error:" + err);
    }
}

async function main() {
    try {
        let franklinDeployedContract = await deployFranklin(wallet, franklinContract);

        let erc20DeployedToken = await erc20token.deployAndAddToFranklin(wallet, franklinDeployedContract);
        
        await ether.deposit("0.1", wallet, franklinDeployedContract);
        await ether.withdraw("0.05", franklinDeployedContract);

        // TODO: - need to add tokens to wallet and send them
        // adding tokens to wallet 
        // await erc20token.deposit(erc20DeployedToken, "0.1", wallet, franklinDeployedContract);
        // await erc20token.withdraw(erc20DeployedToken.address, "0.05", franklinDeployedContract);
        
    } catch (err) {
        console.log("Error:" + err);
    }
}


main();