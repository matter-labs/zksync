import {ethers, Wallet} from "ethers";
import {Deployer, readContractCode, readTestContracts} from "../src.ts/deploy";
import {deployContract} from "ethereum-waffle";
import {parseEther} from "ethers/utils";

(async () => {
    if (process.env.ETH_NETWORK !== "test") {
        console.error("This deploy script is only for localhost-test network");
        process.exit(1);
    }

    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    provider.pollingInterval = 10;

    const deployWallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/0").connect(provider);
    const deployer = new Deployer({deployWallet, contracts: readTestContracts(), verbose: true});
    await deployer.deployAll();
    const governance = deployer.governanceContract(deployWallet);
    await (await governance.setValidator(deployWallet.address, true)).wait();

    const erc20 = await deployContract(
        deployWallet,
        readContractCode("TEST-ERC20"), [],
        {gasLimit: 5000000},
    );
    console.log(`TEST_ERC20=${erc20.address}`);
    await (await governance.addToken(erc20.address)).wait();
    if (await governance.tokenIds(erc20.address) !== 1) {
        console.error("Problem with testkit deployment, TEST_ERC20 token should have id 1");
        process.exit(1);
    }

    for (let i = 0; i < 10; ++i) {
        const testWallet = Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/" + i).connect(provider);
        await(await erc20.mint(testWallet.address, "0x4B3B4CA85A86C47A098A224000000000")).wait();
    }
})();
