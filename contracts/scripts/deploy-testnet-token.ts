import {deployContract} from "ethereum-waffle";
import {ethers, Wallet} from "ethers";
import {readContractCode} from "../src.ts/deploy";
import {publishSourceCodeToEtherscan} from "../src.ts/publish-utils";
import * as fs from "fs";
import {formatUnits, parseUnits} from "ethers/utils";


const mainnetTokens = require(`${process.env.ZKSYNC_HOME}/etc/tokens/mainnet`);

(async () => {
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    const wallet = Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    const contractCode = readContractCode("TestnetERC20Token");

    if (process.env.ETH_NETWORK === "mainnet") {
        throw new Error("Test ERC20 tokens should not be deployed to mainnet");
    }


    const result = [];

    let triedPublishCode = false;
    for (const token of mainnetTokens) {


        const constructorArgs = [`${token.name} (${process.env.ETH_NETWORK})`, token.symbol, token.decimals];

        console.log(`Deploying testnet ERC20: ${constructorArgs.toString()}`);
        const erc20 = await deployContract(
            wallet,
            contractCode, constructorArgs,
            {gasLimit: 800000, gasPrice: parseUnits("100", "gwei")},
        );

        const testnetToken = token;
        testnetToken.address = erc20.address;
        result.push(testnetToken);

        try {
            if (!triedPublishCode) {
                console.log("Publishing source code");
                triedPublishCode = true;
                await publishSourceCodeToEtherscan(
                    erc20.address,
                    "TestnetERC20Token",
                    "",
                    "contracts/test",
                );
            }
        } catch (e) {
            console.log("Error failed to verified code:", e);
        }
    }

    fs.writeFileSync(
        `${process.env.ZKSYNC_HOME}/etc/tokens/${process.env.ETH_NETWORK}.json`,
        JSON.stringify(result, null, 2),
    );
})();
