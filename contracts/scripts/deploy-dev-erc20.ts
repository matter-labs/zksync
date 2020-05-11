import {deployContract} from "ethereum-waffle";
import {ethers, Wallet} from "ethers";
import {parseEther} from "ethers/utils";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

const ERC20MintableContract = function() {
    const contract = require("openzeppelin-solidity/build/contracts/ERC20Mintable");
    contract.evm = {bytecode: contract.bytecode};
    contract.interface = contract.abi;
    return contract;
}();

async function main() {
    const wallet = Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

    const erc20 = await deployContract(
        wallet,
        ERC20MintableContract, [],
        {gasLimit: 5000000},
    );

    await erc20.mint(wallet.address, parseEther("3000000000"));
    for (let i = 0; i < 10; ++i) {
        const testWallet = Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/" + i).connect(provider);
        await erc20.mint(testWallet.address, parseEther("3000000000"));
    }

    console.log(JSON.stringify([{address: erc20.address, precision: 18, symbol: "ERC20-1"}], null, 2));
}

main();
