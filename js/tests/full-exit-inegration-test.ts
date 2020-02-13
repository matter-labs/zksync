import {
    depositFromETH,
    Wallet,
    Provider,
    ETHProxy, getDefaultProvider, types, emergencyWithdraw
} from "zksync";
import { ethers, utils } from "ethers";
import {parseEther} from "ethers/utils";

let syncProvider: Provider;

async function testRandomAccountFullExit(syncWallet: Wallet, token: types.Token) {
    const fullExit = await emergencyWithdraw({
        withdrawFrom: syncWallet,
        token: "ETH",
        accountId: 2
    });
    await fullExit.awaitReceipt();
    console.log(`Full exit random account ok, Token: ${token}`);
}

async function testNormalFullExit(syncWallet: Wallet, token: types.Token) {
    const balanceBeforeWithdraw = await syncWallet.getBalance(token);
    if (balanceBeforeWithdraw.eq(0)) {
        throw new Error("Bug in the test code -- balance should be non 0");
    }
    const fullExit = await emergencyWithdraw({
        withdrawFrom: syncWallet,
        token,
    });
    await fullExit.awaitReceipt();

    const balanceAfterWithdraw = await syncWallet.getBalance(token);
    if (!balanceAfterWithdraw.eq(0)) {
        throw new Error("Balance after withdraw not zero");
    }
    console.log(`Full exit success ok, Token: ${token}`);
}

async function testEmptyBalanceFullExit(syncWallet: Wallet, token: types.Token) {
    const balanceBeforeWithdraw = await syncWallet.getBalance(token);
    if (!balanceBeforeWithdraw.eq(0)) {
        throw new Error("Bug in the test code -- balance should be 0");
    }
    const fullExit = await emergencyWithdraw({
        withdrawFrom: syncWallet,
        token,
    });
    await fullExit.awaitReceipt();

    const balanceAfterWithdraw = await syncWallet.getBalance(token);
    if (!balanceAfterWithdraw.eq(0)) {
        throw new Error("Balance after withdraw not zero");
    }
    console.log(`Full exit empty balance ok, Token: ${token}`);
}

async function testWrongETHWalletFullExit(ethWallet: ethers.Wallet, syncWallet: Wallet, token: types.Token) {
    const balanceBeforeWithdraw = await syncWallet.getBalance(token);
    if (balanceBeforeWithdraw.eq(0)) {
        throw new Error("Bug in the test code -- balance should not be 0");
    }

    // post emergency withdraw with wrong wallet.
    const oldWallet = syncWallet.ethSigner;
    syncWallet.ethSigner = ethWallet;
    const fullExit = await emergencyWithdraw({
        withdrawFrom: syncWallet,
        token,
        nonce: 12341
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
    const WEB3_URL = process.env.WEB3_URL;
// Mnemonic for eth wallet.
    const MNEMONIC = process.env.MNEMONIC;
    const ERC_20TOKEN = process.env.TEST_ERC20;
    const network = process.env.ETH_NETWORK == "localhost" ? "localhost" : "testnet";
    console.log("Running integration test on the ", network, " network");

    syncProvider = await getDefaultProvider(network);

    const ethersProvider = new ethers.providers.JsonRpcProvider(WEB3_URL);
    const ethProxy = new ETHProxy(ethersProvider, syncProvider.contractAddress);

    const ethWallet = ethers.Wallet.fromMnemonic(
        MNEMONIC,
        "m/44'/60'/0'/0/1"
    ).connect(ethersProvider);


    for (let token of ["ETH", ERC_20TOKEN]) {
        let amount = utils.parseEther("0.089");
        const ethWallet2 = ethers.Wallet.createRandom().connect(ethersProvider);
        const syncWallet2 = await Wallet.fromEthSigner(
            ethWallet2,
            syncProvider,
            ethProxy
        );
        await (await ethWallet.sendTransaction({to: ethWallet2.address, value: parseEther("0.5")})).wait();

        await testRandomAccountFullExit(syncWallet2, token);
        const deposit = await depositFromETH({
            depositFrom: ethWallet,
            depositTo: syncWallet2,
            token,
            amount,
        });
        await deposit.awaitReceipt();
        await testWrongETHWalletFullExit(ethWallet, syncWallet2, token);
        await testNormalFullExit(syncWallet2, token);
        await testEmptyBalanceFullExit(syncWallet2, token);
    }


    await syncProvider.disconnect();
})();
