import {
    depositFromETH,
    Wallet,
    Provider,
    ETHProxy, getDefaultProvider, types, emergencyWithdraw
} from "zksync";
import { ethers, utils } from "ethers";

let syncProvider: Provider;

async function testRandomAccountFullExit(ethWallet: ethers.Wallet, syncWallet: Wallet, token: types.Token) {
    const fullExit = await emergencyWithdraw({
        withdrawTo: ethWallet,
        withdrawFrom: syncWallet,
        token: "ETH",
        accountId: 2
    });
    await fullExit.awaitVerifyReceipt();
    console.log(`Full exit random account ok, Token: ${token}`);
}

async function testNormalFullExit(ethWallet: ethers.Wallet, syncWallet: Wallet, token: types.Token) {
    const balanceBeforeWithdraw = await syncWallet.getBalance(token);
    if (balanceBeforeWithdraw.eq(0)) {
        throw new Error("Bug in the test code -- balance should be non 0");
    }
    const fullExit = await emergencyWithdraw({
        withdrawTo: ethWallet,
        withdrawFrom: syncWallet,
        token,
    });
    await fullExit.awaitVerifyReceipt();

    const balanceAfterWithdraw = await syncWallet.getBalance(token);
    if (!balanceAfterWithdraw.eq(0)) {
        throw new Error("Balance after withdraw not zero");
    }
    console.log(`Full exit success ok, Token: ${token}`);
}

async function testEmptyBalanceFullExit(ethWallet: ethers.Wallet, syncWallet: Wallet, token: types.Token) {
    const balanceBeforeWithdraw = await syncWallet.getBalance(token);
    if (!balanceBeforeWithdraw.eq(0)) {
        throw new Error("Bug in the test code -- balance should be 0");
    }
    const fullExit = await emergencyWithdraw({
        withdrawTo: ethWallet,
        withdrawFrom: syncWallet,
        token,
    });
    await fullExit.awaitVerifyReceipt();

    const balanceAfterWithdraw = await syncWallet.getBalance(token);
    if (!balanceAfterWithdraw.eq(0)) {
        throw new Error("Balance after withdraw not zero");
    }
    console.log(`Full exit empty balance ok, Token: ${token}`);
}

async function testWrongNonceFullExit(ethWallet: ethers.Wallet, syncWallet: Wallet, token: types.Token) {
    const balanceBeforeWithdraw = await syncWallet.getBalance(token);
    if (balanceBeforeWithdraw.eq(0)) {
        throw new Error("Bug in the test code -- balance should not be 0");
    }
    const fullExit = await emergencyWithdraw({
        withdrawTo: ethWallet,
        withdrawFrom: syncWallet,
        token,
        nonce: 12341
    });
    await fullExit.awaitVerifyReceipt();

    const balanceAfterWithdraw = await syncWallet.getBalance(token);
    if (!balanceAfterWithdraw.eq(balanceBeforeWithdraw)) {
        throw new Error("Balance after withdraw not zero");
    }
    console.log(`Full exit wrong nonce ok, Token: ${token}`);
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

        let amount = utils.parseEther("0.89");
        const ethWallet2 = ethers.Wallet.createRandom().connect(ethersProvider);
        const syncWallet2 = await Wallet.fromEthSigner(
            ethWallet2,
            syncProvider,
            ethProxy
        );

        await testRandomAccountFullExit(ethWallet, syncWallet2, token);
        const deposit = await depositFromETH({
            depositFrom: ethWallet,
            depositTo: syncWallet2,
            token,
            amount,
        });
        await deposit.awaitReceipt();
        await testWrongNonceFullExit(ethWallet, syncWallet2, token);
        await testNormalFullExit(ethWallet, syncWallet2, token);
        await testEmptyBalanceFullExit(ethWallet, syncWallet2, token);
    }


    await syncProvider.disconnect();
})();
