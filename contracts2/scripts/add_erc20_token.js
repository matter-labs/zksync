// import Contract2 from './build/Franklin'

const FrankliContract = require('../build/Franklin');
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
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    let wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

    let contract = new ethers.Contract(process.env.CONTRACT2_ADDR, FrankliContract.abi, provider).connect(wallet);
    console.log("Franklin address:",contract.address);

    let erc20 = await deployContract(wallet, ERC20MintableContract, []);
    console.log("Test ERC20 address:", erc20.address);
    await contract.addToken(erc20.address)
}


main();
