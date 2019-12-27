const ethers = require("ethers");
const zksync = require("zksync");

async function main() {
    const ethWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1");
    const syncWallet = await zksync.Wallet.fromEthSigner(ethWallet);
    console.log(`OPERATOR_PRIVATE_KEY=${ethWallet.privateKey.toString().slice(2)}`);
    console.log(`OPERATOR_ETH_ADDRESS=${ethWallet.address}`);
    console.log(`OPERATOR_FRANKLIN_ADDRESS=${syncWallet.address()}`);
    console.log(`OPERATOR_ETH_ACCOUNT_PASSWORD="${process.env.MNEMONIC.split(' ').slice(0, 3).join(' ')}"`);
}

main();
