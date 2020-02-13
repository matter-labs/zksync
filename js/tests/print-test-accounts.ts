import {ethers} from "ethers";
import {Wallet} from "zksync";

(async () => {
    const num_test_wallets = 5;
    const baseWalletPath = "m/44'/60'/0'/0/";

    const walletKeys = [];
    for (let i = 0; i < num_test_wallets; ++i) {
        const ethWallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, baseWalletPath + i)
        walletKeys.push({
            address: ethWallet.address,
            privateKey: ethWallet.privateKey,
        })
    }
    console.log(JSON.stringify(walletKeys));
})();