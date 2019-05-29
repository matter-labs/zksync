const ethers = require('ethers')

const bn = ethers.utils.bigNumberify;

const gasPriceScaling = bn(12).add(bn(1));

async function rescue() {
    console.log("This is intended to run on mainnet only!");
    const web3Url = process.env.WEB3_URL;
    let privateKey = process.env.PRIVATE_KEY;
    const saveAddress = process.env.FUNDING_ADDR;

    // const web3Url = "http://localhost:8545";
    // let privateKey = "27593fea79697e947890ecbecce7901b0008345e5d7259710d0dd5e500d040be";
    if (privateKey === undefined || web3Url === undefined) {
        console.log("Missing private key or web3 URL in environment");
        return;
    }
    if (! privateKey.startsWith("0x")) {
        privateKey = "0x" + privateKey;
    }
    const provider = new ethers.providers.JsonRpcProvider(web3Url);
    const source = new ethers.Wallet(privateKey, provider);
    const address = source.address, saveAddress

    console.log(address)
    //process.exit(0)

    source.connect(provider);

    let gasPrice = await provider.getGasPrice();
    console.log("Current gas price is " + gasPrice.div(bn(1000000000)).toString() + " GWei");

    gasPrice = gasPrice.mul(gasPriceScaling);

    let latestNonce = await provider.getTransactionCount(address, "latest");
    let pendingNonce = await provider.getTransactionCount(address, "pending");
    let balance = await provider.getBalance(address, "pending");

    console.log('Nonce: latest = ', latestNonce, ', pending = ', pendingNonce, ', pending balance = ', ethers.utils.formatEther(balance));
    console.log('Saving funds to', saveAddress);

    // if (latestNonce === pendingNonce) {
    //     console.log("No transactions to replace");
    //     return;
    // }

    for (let i = 218; i <= 218 + 10; i++) {
        console.log("Replacing nonce = " + i);
        try {
            let gasLimit = 21000
            let value = ethers.utils.parseEther('1.8') //balance.sub(gasPrice.mul(gasLimit)).sub(ethers.utils.parseEther('0.1'))
            let result = await source.sendTransaction(
                {
                    to: address,
                    nonce: i,
                    gasPrice,
                    gasLimit,
                    //value,
                }
            );
            console.log("Successfully send with hash " + result.hash);
            console.log("Used gas price " + gasPrice.div(bn(1000000000)).toString() + " GWei and limit 21000");
        } catch(error) {
            if (error.transactionHash !== undefined) {
                console.log("There may have been a network erro sending transaction, replacements hash = " + error.transactionHash);
                console.log('Reason:', error.reason);
            } else {
                console.log(error);
            }
        }
    }

}

rescue().then(() => {
    console.log("Done");
});