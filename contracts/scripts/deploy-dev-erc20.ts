import {deployContract} from "ethereum-waffle";
import {ethers, Wallet} from "ethers";
import {readContractCode} from "../src.ts/deploy";
import {parseEther} from "ethers/lib/utils";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

async function main() {
    const wallet = Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

    let name = process.argv[2];
    let symbol = process.argv[3];
    let decimals = Number(process.argv[4]);

    const erc20 = await deployContract(
       wallet,
       readContractCode("TestnetERC20Token"), [name, symbol, decimals],
       {gasLimit: 5000000},
    );

    await erc20.mint(wallet.address, parseEther("3000000000"));
    for (let i = 0; i < 10; ++i) {
        const testWallet = Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/" + i).connect(provider);
        await erc20.mint(testWallet.address, parseEther("3000000000"));
    }
    const result = {address: erc20.address, name: name, decimals: decimals, symbol: symbol};
    console.log(JSON.stringify(result, null, 2));
}

main();
