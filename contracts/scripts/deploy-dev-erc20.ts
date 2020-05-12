import {deployContract} from "ethereum-waffle";
import {ethers, Wallet} from "ethers";
import {parseEther} from "ethers/utils";
import {readContractCode} from "../src.ts/deploy";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

async function main() {
    const wallet = Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

    const result = [];

    for (const token of [{symbol: "ERC-1", precision: 18}, {symbol: "ERC-2", precision: 6}]) {
        const erc20 = await deployContract(
            wallet,
            readContractCode("TEST-ERC20"), [],
            {gasLimit: 5000000},
        );

        await erc20.mint(wallet.address, parseEther("3000000000"));
        for (let i = 0; i < 10; ++i) {
            const testWallet = Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/" + i).connect(provider);
            await erc20.mint(testWallet.address, parseEther("3000000000"));
        }
        result.push({address: erc20.address, precision: token.precision, symbol: token.symbol});
    }

    console.log(JSON.stringify(result, null, 2));
}

main();
