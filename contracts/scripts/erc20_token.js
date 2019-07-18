// import Contract2 from './build/Franklin'

const {
    deployContract
} = require('ethereum-waffle');

async function deployAndAddToFranklin(tokenContract, wallet, franklinContract) {
    try {
        let erc20 = await deployContract(wallet, tokenContract, []);
        console.log("Test ERC20 address:", erc20.address);
        await franklinContract.addToken(erc20.address)
    } catch (err) {
        console.log("Error:", err);
    }
}

module.exports = {
    deployAndAddToFranklin
}
