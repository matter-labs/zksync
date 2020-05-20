import {
    Wallet,
    Provider,
    ETHProxy, getDefaultProvider, types
} from "zksync";
import { ethers, utils } from "ethers";
import {parseEther} from "ethers/utils";

let syncProvider: Provider;

async function testRandomAccountFullExit(syncWallet: Wallet, token: types.TokenLike) {
    const fullExit = await syncWallet.emergencyWithdraw({
        token: "ETH",
        accountId: 2
    });
    await fullExit.awaitReceipt();
    console.log(`Full exit random account ok, Token: ${token}`);
}

async function testNormalFullExit(syncWallet: Wallet, token: types.TokenLike) {
    const balanceBeforeWithdraw = await syncWallet.getBalance(token);
    if (balanceBeforeWithdraw.eq(0)) {
        throw new Error("Bug in the test code -- balance should be non 0");
    }
    const fullExit = await syncWallet.emergencyWithdraw({
        token,
    });
    await fullExit.awaitReceipt();

    const balanceAfterWithdraw = await syncWallet.getBalance(token);
    if (!balanceAfterWithdraw.eq(0)) {
        throw new Error("Balance after withdraw not zero");
    }
    console.log(`Full exit success ok, Token: ${token}`);
}

async function testEmptyBalanceFullExit(syncWallet: Wallet, token: types.TokenLike) {
    const balanceBeforeWithdraw = await syncWallet.getBalance(token);
    if (!balanceBeforeWithdraw.eq(0)) {
        throw new Error("Bug in the test code -- balance should be 0");
    }
    const fullExit = await syncWallet.emergencyWithdraw({
        token,
    });
    await fullExit.awaitReceipt();

    const balanceAfterWithdraw = await syncWallet.getBalance(token);
    if (!balanceAfterWithdraw.eq(0)) {
        throw new Error("Balance after withdraw not zero");
    }
    console.log(`Full exit empty balance ok, Token: ${token}`);
}

async function testWrongETHWalletFullExit(ethWallet: ethers.Wallet, syncWallet: Wallet, token: types.TokenLike) {
    const balanceBeforeWithdraw = await syncWallet.getBalance(token);
    if (balanceBeforeWithdraw.eq(0)) {
        throw new Error("Bug in the test code -- balance should not be 0");
    }

    // post emergency withdraw with wrong wallet.
    const oldWallet = syncWallet.ethSigner;
    syncWallet.ethSigner = ethWallet;
    const fullExit = await syncWallet.emergencyWithdraw({
        token,
    });
    await fullExit.awaitReceipt();
    syncWallet.ethSigner = oldWallet;

    const balanceAfterWithdraw = await syncWallet.getBalance(token);
    if (!balanceAfterWithdraw.eq(balanceBeforeWithdraw)) {
        throw new Error("Balance after withdraw not equal to balance before withdraw");
    }
    console.log(`Full exit wrong eth account ok, Token: ${token}`);
}

(async () => {
    try {
        const WEB3_URL = process.env.WEB3_URL;
// Mnemonic for eth wallet.
        const MNEMONIC = process.env.MNEMONIC;
        const network = process.env.ETH_NETWORK == "localhost" ? "localhost" : "testnet";
        console.log("Running integration test on the ", network, " network");

        syncProvider = await Provider.newWebsocketProvider(process.env.WS_API_ADDR);
        const ERC_20TOKEN = syncProvider.tokenSet.resolveTokenAddress("ERC20-1");

        const ethersProvider = new ethers.providers.JsonRpcProvider(WEB3_URL);

        const ethWallet = ethers.Wallet.fromMnemonic(
            MNEMONIC,
            "m/44'/60'/0'/0/1"
        ).connect(ethersProvider);
        const depositWallet = await Wallet.fromEthSignerNoKeys(ethWallet, syncProvider);


        for (let token of ["ETH", ERC_20TOKEN]) {
            let amount = utils.parseEther("0.089");
            const ethWallet2 = ethers.Wallet.createRandom().connect(ethersProvider);
            const syncWallet2 = await Wallet.fromEthSigner(
                ethWallet2,
                syncProvider,
            );
            await (await ethWallet.sendTransaction({to: ethWallet2.address, value: parseEther("0.5")})).wait();

            await testRandomAccountFullExit(syncWallet2, token);
            const deposit = await depositWallet.depositToSyncFromEthereum({
                depositTo: syncWallet2.address(),
                token,
                amount,
                approveDepositAmountForERC20: true,
            });
            await deposit.awaitReceipt();
            await testWrongETHWalletFullExit(ethWallet, syncWallet2, token);
            await testNormalFullExit(syncWallet2, token);
            await testEmptyBalanceFullExit(syncWallet2, token);
        }
        await syncProvider.disconnect();
    } catch (e) {
        console.error("Error:", e);
        process.exit(1);
    }
})();
