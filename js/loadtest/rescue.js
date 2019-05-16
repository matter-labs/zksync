const ethers = require('ethers')

const bn = ethers.utils.bigNumberify;

const gasPriceScaling = bn(20);

async function rescue() {
    console.log("This is intended to run on mainnet only!");
    const web3Url = process.env.WEB3_URL;
    let privateKey = process.env.PRIVATE_KEY;

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
    const address = source.address;
    source.connect(provider);

    let gasPrice = await provider.getGasPrice();
    console.log("Current gas price is " + gasPrice.div(bn(1000000000)).toString() + " GWei");

    gasPrice = gasPrice.mul(gasPriceScaling);

    let latestNonce = await provider.getTransactionCount(address, "latest");
    let pendingNonce = await provider.getTransactionCount(address, "pending");

    if (latestNonce === pendingNonce) {
        console.log("No transactions to replace");
        return;
    }

    for (let i = latestNonce; i <= pendingNonce; i++) {
        console.log("Replacing nonce = " + i);
        try {
            let result = await source.sendTransaction(
                {
                    to: address,
                    nonce: i,
                    gasPrice: gasPrice,
                    gasLimit: 21000,
                }
            );
            console.log("Successfully send with hash " + result.hash);
            console.log("Used gas price " + gasPrice.div(bn(1000000000)).toString() + " GWei and limit 21000");
        } catch(error) {
            if (error.transactionHash !== undefined) {
                console.log("There may have been a network erro sending transaction, replacements hash = " + error.transactionHash);
            } else {
                console.log(error);
            }
        }
    }

}

rescue().then(() => {
    console.log("Done");
});