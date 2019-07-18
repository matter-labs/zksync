// import Contract2 from './build/Franklin'

const franklinContract = require('../build/Franklin');
const {
    deployContract
} = require('ethereum-waffle');
const ethers = require('ethers');
const ERC20MintableContract = function () {
    let contract = require('openzeppelin-solidity/build/contracts/ERC20Mintable');
    contract.evm = {bytecode: contract.bytecode};
    return contract
}();
const erc20token = require('./erc20_token.js');

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider); ////"fine music test violin matrix prize squirrel panther purchase material script deal"

async function deployFranklin(wallet, franklinContract) {
    try {
        let contract = await deployContract(wallet, franklinContract, [ethers.constants.HashZero, wallet.address, wallet.address], {
            gasLimit: 8000000
        });
        console.log("Franklin address:",contract.address);

        return contract
    } catch (err) {
        console.log("Error:", err);
    }
}


async function main() {
    try {
        let franklinDeployedContract = await deployFranklin(wallet, franklinContract);
        let erc20Token = await erc20token.deployAndAddToFranklin(ERC20MintableContract, wallet, franklinDeployedContract);
    } catch (err) {
        console.log("Error:", err);
    }
}


main();