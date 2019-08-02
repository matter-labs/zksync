const ethers = require("ethers");
const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

const rinkeby = ethers.getDefaultProvider("rinkeby");
const mainnet = new ethers.providers.InfuraProvider();
const mainnet2 = new ethers.providers.EtherscanProvider();

async function calc(addr) {
    const balanceBefore = await provider.getBalance(addr, 4385572);
    const balanceAfter = await provider.getBalance(addr, 4386335);
    console.log("balanceBefore:", ethers.utils.formatEther(balanceBefore));
    console.log("balanceAfter:", ethers.utils.formatEther(balanceAfter));
    console.log("diff:", ethers.utils.formatEther(balanceBefore.sub(balanceAfter)));
}

async function main() {

    console.log("gas price rinkeby", (await provider.getGasPrice()).toNumber() );
    console.log("gas price rinkeby", (await rinkeby.getGasPrice()).toNumber() );

    console.log("gas price mainnet", (await mainnet.getGasPrice()).toNumber() );
    console.log("gas price mainnet2", (await mainnet2.getGasPrice()).toNumber() );

    calc("0x" + process.env.SENDER_ACCOUNT);
    calc("0xB0587796F36E39c4b0d79790D2Efa874386dcD6d");
}

main();
