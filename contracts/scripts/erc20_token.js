const ethers = require('ethers');
const {
    deployContract
} = require('ethereum-waffle');
const ERC20MintableContract = function () {
    let contract = require('openzeppelin-solidity/build/contracts/ERC20Mintable');
    contract.evm = {bytecode: contract.bytecode};
    return contract
}();

async function deployAndAddToFranklin(wallet, franklin) {
    try {
        let erc20 = await deployContract(wallet, ERC20MintableContract, []);
        console.log("Test ERC20 address:" + erc20.address);
        await franklin.addToken(erc20.address)
        return erc20
    } catch (err) {
        console.log("Error:" + err);
    }
}

async function addToFranklin(tokenAddress, franklin) {
    try {
        await franklin.addToken(tokenAddress)
    } catch (err) {
        console.log("Error:" + err);
    }
}

async function deposit(token, amount, wallet, franklin) {
    try {
        if (amount == 0) {
            throw "Sending nothing is meanless";
        }
        let balance = await token.balanceOf(wallet.address);
        let depositAmount = ethers.utils.parseEther(amount);
        if (balance <= depositAmount) {
            throw "Too much tokens sending";
        }

        let tx = await franklin.depositERC20(token.address, depositAmount);

        console.log("Sent token in Transaction: " + tx.hash);
    } catch (error) {
        console.log("Error in sending token:" + error);
    }
}

// TODO: - dont work
async function withdraw(tokenAddress, amount, franklin) {
    try {
        let withdrawAmount = ethers.utils.parseEther(amount);
        // let overrides = {
        //     gasLimit: 10000000
        // };
        let tx = await franklin.withdrawERC20(tokenAddress, withdrawAmount);

        console.log("Withdrew token in Transaction: " + tx.hash);
    } catch (error) {
        console.log("Error in withdrawing token:" + error);
    }
}

module.exports = {
    deployAndAddToFranklin,
    addToFranklin,
    deposit,
    withdraw
}
