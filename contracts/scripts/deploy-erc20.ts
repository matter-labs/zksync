import { Command } from "commander";
import { deployContract } from "ethereum-waffle";
import { ethers, Wallet } from "ethers";
import { readContractCode } from "../src.ts/deploy";
import { parseEther } from "ethers/lib/utils";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

type Token = {
    address: string | null;
    name: string;
    symbol: string;
    decimals: number;
};

async function deployToken(token: Token): Promise<Token> {
    const erc20 = await deployContract(
        wallet,
        readContractCode("TestnetERC20Token"),
        [token.name, token.symbol, token.decimals],
        { gasLimit: 5000000 }
    );

    await erc20.mint(wallet.address, parseEther("3000000000"));
    for (let i = 0; i < 10; ++i) {
        const testWallet = Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/" + i).connect(provider);
        await erc20.mint(testWallet.address, parseEther("3000000000"));
    }
    token.address = erc20.address;

    return token;
}

async function main() {
    const program = new Command();

    program
        .version("0.1.0")
        .name("deploy-erc20")
        .description("deploy testnet erc20 token");

    program
        .command("add")
        .option("-n, --name <name>")
        .option("-s, --symbol <symbol>")
        .option("-d, --decimals <decimals>")
        .description("Adds a new token with a given fields")
        .action(async (name: string, symbol: string, decimals: number) => {
            const token: Token = { address: null, name, symbol, decimals };
            console.log(JSON.stringify(deployToken(token), null, 2));
        });

    program
        .command("add-multi <tokens_json>")
        .description("Adds a multiple tokens given in JSON format")
        .action(async (tokens_json: string) => {
            const tokens: Array<Token> = JSON.parse(tokens_json);
            const result = [];

            for (const token of tokens) {
                result.push(await deployToken(token));
            }

            console.log(JSON.stringify(result, null, 2));
        });

    program.parse(process.argv);
}

main();
