const ethers = require("ethers");
const { bigNumberify, parseEther } = ethers.utils;

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

async function createWallets() {
    while (true) {
        try {
            console.log('connecting to provider...');

            const ethersProvider = new ethers.providers.JsonRpcProvider('http://localhost:7545');
            await ethersProvider.getBlockNumber()
            const baseWalletPath = "m/44'/60'/0'/0/";
            
            // this is the wallet with a lot of funds. 99 because we have 100 wallets with funds
            const wallet1 = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, baseWalletPath + 99).connect(ethersProvider);

            // these are wallets we aren't likely to use.
            const wallet2 = ethers.Wallet.createRandom().connect(ethersProvider);
            const wallet3 = ethers.Wallet.createRandom().connect(ethersProvider);
            return [ wallet1, wallet2, wallet3 ];
        } catch (e) {
            await sleep(1000);
        }
    }
}

async function generateBlocks() {
    const [ wallet1, wallet2, wallet3 ] = await createWallets();

    await wallet1
        .sendTransaction({ to: wallet2.address, value: parseEther("10") })
        .then(tx => tx.wait());

    const blockGenerationIntervalMillis = 100;
    while (true) {
        await wallet2.sendTransaction({ to: wallet3.address, value: bigNumberify(1) });
        await sleep(blockGenerationIntervalMillis);
    }
}

generateBlocks();
