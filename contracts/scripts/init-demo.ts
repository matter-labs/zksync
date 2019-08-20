import {Contract, ethers} from "ethers";
import {parseEther} from "ethers/utils";

export const IERC20 = function () {
    let contract = require('openzeppelin-solidity/build/contracts/IERC20');
    contract.evm = {bytecode: contract.bytecode};
    return contract
}();

async function main() {
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    const erc20DeployedToken = new Contract(process.env.TEST_ERC20, IERC20.abi, provider);
    const walletRich = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    const walletOne = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/4").connect(provider);
    const walletTwo = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/3").connect(provider);

    console.log("walletOnePrivKey ",walletOne.privateKey);
    console.log("walletTwoPrivKey ",walletTwo.privateKey);

    await walletRich.sendTransaction({to: walletOne.address, value: parseEther("100")});
    await walletRich.sendTransaction({to: walletTwo.address, value: parseEther("5")});
    let tx = await erc20DeployedToken.connect(walletRich).transfer(walletOne.address, 250);
    await tx.wait();

    console.log(await erc20DeployedToken.balanceOf(walletOne.address));
    console.log(await erc20DeployedToken.balanceOf(walletTwo.address));
}

main();
