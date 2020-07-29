import {
    Provider, Wallet,
} from "zksync";
import {ethers} from "ethers";
import {parseEther} from "ethers/utils";

const WEB3_URL = process.env.WEB3_URL;

const network = process.env.ETH_NETWORK == "localhost" ? "localhost" : "stage";
const ethersProvider = new ethers.providers.JsonRpcProvider(WEB3_URL);
if (network == "localhost") {
    ethersProvider.pollingInterval = 100;
}

let syncProvider: Provider;

(async () => {
    try {
        syncProvider = await Provider.newWebsocketProvider(process.env.WS_API_ADDR);

        // PARAMS
        const depositAmount = parseEther("100000000000") // 10^11
        const ERC20_SYMBOL = "MLTT";
        const ERC20_ID = syncProvider.tokenSet.resolveTokenId(ERC20_SYMBOL);


        console.log("Token symbol:", ERC20_SYMBOL);
        console.log("Token Id:", ERC20_ID)
        console.log("Token address:", syncProvider.tokenSet.resolveTokenAddress(ERC20_SYMBOL));

        const ethWallet = ethers.Wallet.fromMnemonic(
            process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/5"
        ).connect(ethersProvider);
        const syncWallet = await Wallet.fromEthSigner(ethWallet, syncProvider);
        console.log("Wallet address:", ethWallet.address);
        console.log("Wallet ethereum private key:", ethWallet.privateKey);
        console.log("Wallet sync private key:", Buffer.from(syncWallet.signer.privateKey).toString("hex"));
        console.log("Wallet sync pubkey hash:", syncWallet.signer.pubKeyHash());

        const deposit = await syncWallet.depositToSyncFromEthereum({
            depositTo: syncWallet.address(),
            token: ERC20_SYMBOL,
            amount: depositAmount,
            approveDepositAmountForERC20: true
        });
        await deposit.awaitReceipt();
        console.log("Deposit success");

        if (!await syncWallet.isSigningKeySet()) {
            const changePubkey = await syncWallet.setSigningKey();
            await changePubkey.awaitReceipt();
        }
        console.log("Pubkey set success");

        process.exit(0);
    } catch (e) {
        console.error("Error: ", e);
        process.exit(1);
    }
})();
