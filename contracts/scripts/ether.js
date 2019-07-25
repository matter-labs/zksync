const ethers = require('ethers');

async function deposit(amount, wallet, franklin) {
    try {
        if (amount == 0) {
            throw "Sending nothing is meanless";
        }
        let balance = await wallet.getBalance();
        let depositAmount = ethers.utils.parseEther(amount);
        if (balance <= depositAmount) {
            throw "Too much ethers sending";
        }

        let tx = await wallet.sendTransaction({
            to: franklin.address,
            value: depositAmount
        });

        console.log("Sent ether in Transaction: " + tx.hash);
    } catch (error) {
        console.log("Error in sending ether:" + error);
    } 
}

// TODO: - dont work
async function withdraw(amount, franklin) {
    try {
        let withdrawAmount = ethers.utils.parseEther(amount);
        let overrides = {
            gasLimit: 10000000
        };
        let tx = await franklin.withdrawETH(withdrawAmount, overrides);

        console.log("Withdrew ether in Transaction: " + tx.hash);
    } catch (error) {
        console.log("Error in withdrawing ether:" + error);
    }
}

module.exports = {
    deposit,
    withdraw
}
