import {ethers} from "ethers";
import * as zksync from "zksync";

const DEPOSIT_AMOUNT = ethers.utils.parseEther("10000000000");

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const deployerEthWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const faucetEthWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);

async function main() {
    const syncProvider = await zksync.Provider.newHttpProvider(process.env.HTTP_RPC_API_ADDR);
    const deployerWallet = await zksync.Wallet.fromEthSigner(deployerEthWallet, syncProvider);
    const faucetWallet = await zksync.Wallet.fromEthSigner(faucetEthWallet, syncProvider);

    console.log("Faucet ETH_PRIVATE_KEY", faucetEthWallet.privateKey);
    const TOKEN_ADDRESS = syncProvider.tokenSet.resolveTokenAddress("MLTT");
    const ABI = [{
        "constant": false,
        "inputs": [
            {
                "internalType": "address",
                "name": "_to",
                "type": "address"
            },
            {
                "internalType": "uint256",
                "name": "_amount",
                "type": "uint256"
            }
        ],
        "name": "mint",
        "outputs": [
            {
                "internalType": "bool",
                "name": "",
                "type": "bool"
            }
        ],
        "payable": false,
        "stateMutability": "nonpayable",
        "type": "function"
    }];
    if (process.env.NETWORK !== "localhost") {
        const erc20Mintable = new ethers.Contract(TOKEN_ADDRESS, ABI, deployerEthWallet);
        const mintTx = await erc20Mintable.mint(deployerEthWallet.address, DEPOSIT_AMOUNT);
        await mintTx.wait();
        console.log("Mint successful");
    }

    const deposit = await deployerWallet.depositToSyncFromEthereum({
        depositTo: faucetEthWallet.address,
        token: "MLTT",
        amount: DEPOSIT_AMOUNT,
        approveDepositAmountForERC20: true
    });
    await deposit.awaitReceipt();
    console.log("Deposit successful");

    if (! await faucetWallet.isSigningKeySet()) {
        const setSigningKey = await faucetWallet.setSigningKey({ feeToken: "MLTT" });
        await setSigningKey.awaitReceipt();
        console.log("Signing key is set");
    }
    console.log("Faucet account is prepared");
}

main();
