import {ArgumentParser} from "argparse";
import {deployContract} from "ethereum-waffle";
import {ethers, Wallet} from "ethers";
import {readContractCode} from "../src.ts/deploy";
import {parseEther} from "ethers/lib/utils";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

async function main() {
    const parser = new ArgumentParser({
        version: "0.1.0",
        addHelp: true,
        description: "deploy testnet erc20 token",
    });

    const wallet = Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    
    parser.addArgument("--name", {required: true, help: "Name erc20 token"});
    parser.addArgument("--symbol", {required: true, help: "Symbol erc20 token"});
    parser.addArgument("--decimals", {required: true, help: "Decimals erc20 token", type: Number});
    
    const args = parser.parseArgs(process.argv.slice(2));

    const erc20 = await deployContract(
       wallet,
       readContractCode("TestnetERC20Token"), [args.name, args.symbol, args.decimals],
       {gasLimit: 5000000},
    );

    await erc20.mint(wallet.address, parseEther("3000000000"));
    for (let i = 0; i < 10; ++i) {
        const testWallet = Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/" + i).connect(provider);
        await erc20.mint(testWallet.address, parseEther("3000000000"));
    }
    const result = {address: erc20.address, name: args.name, decimals: args.decimals, symbol: args.symbol};
    console.log(JSON.stringify(result, null, 2));
}

main();
