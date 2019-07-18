// import Contract2 from './build/Franklin'

const FranklinContract = require('../build/Franklin');
const {
    deployContract
} = require('ethereum-waffle');
const ethers = require('ethers');
const ERC20MintableContract = function () {
    let contract = require('openzeppelin-solidity/build/contracts/ERC20Mintable');
    contract.evm = {bytecode: contract.bytecode};
    return contract
}();


async function main() {
    try {
        const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
        let wallet = ethers.Wallet.fromMnemonic("fine music test violin matrix prize squirrel panther purchase material script deal").connect(provider); //"fine music test violin matrix prize squirrel panther purchase material script deal"

        let contract = await deployContract(wallet, FranklinContract, [ethers.constants.HashZero, wallet.address, wallet.address], {
            gasLimit: 8000000
        });
        console.log("Franklin address:",contract.address);

        // let erc20 = await deployContract(wallet, ERC20MintableContract, []);
        // console.log("Test ERC20 address:", erc20.address);
        // await contract.addToken(erc20.address)
    } catch (err) {
        console.log("Error:", err);
    }
}


main();