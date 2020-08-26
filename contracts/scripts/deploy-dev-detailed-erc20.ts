import {ethers} from "ethers";
import {readContractCode} from "../src.ts/deploy";
import {deployContract} from "ethereum-waffle";

(async () => {
    if (process.env.ETH_NETWORK !== "test") {
        console.error("This deploy script is only for localhost-test network");
        process.exit(1);
    }
    
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    provider.pollingInterval = 10;

    const deployWallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/0").connect(provider);
    
    let name = process.argv[2];
    let symbol = process.argv[3];
    let decimals = Number(process.argv[4]);
    console.log(name, symbol, decimals);
    
    const erc20 = await deployContract(
        deployWallet,
        readContractCode("TEST-FULL-ERC20"), [name, symbol, decimals],
        {gasLimit: 5000000},
    );
    
    console.log(`TEST_FULL_ERC20=${erc20.address}`);
})();
