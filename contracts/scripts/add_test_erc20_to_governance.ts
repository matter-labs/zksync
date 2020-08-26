import {ethers, Wallet, BigNumber} from "ethers";
import {Deployer, readTestContracts} from "../src.ts/deploy";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

(async () => {
    let tokenAddress = process.argv[2];
    let deployerPrivateKey = process.argv[3];

    const deployer = new Deployer({deployWallet: ethers.Wallet.createRandom()});
    const governorWallet = new Wallet(deployerPrivateKey, provider);

    console.log("Adding new ERC20 token to network: ", tokenAddress);
    const tx = await deployer
        .governanceContract(governorWallet)
        .addToken(tokenAddress, {gasLimit: BigNumber.from("1000000")});
        
    console.log("tx hash: ", tx.hash);
    const receipt = await tx.wait();
    console.log("status: ", receipt.status);
})();
