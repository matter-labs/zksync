import * as zksync from "../src/index";
import { Contract, ethers, utils } from "ethers";
import { formatEther, parseEther } from "ethers/utils";
import { emergencyWithdraw } from "../src/index";

const WEB3_URL = process.env.WEB3_URL;
// Mnemonic for eth wallet.
const MNEMONIC = process.env.TEST_MNEMONIC;
const TOKEN = process.env.TEST_ERC20;
const network =
    process.env.ETH_NETWORK == "localhost" ? "localhost" : "testnet";

function shortAddr(address: string): string {
    return `${address.substr(0, 6)}`;
}

async function logSyncBalance(
    wallet: zksync.Wallet,
    token: zksync.types.TokenLike,
    type: "committed" | "verified" = "committed"
) {
    const balance = formatEther(await wallet.getBalance(token, type));
    console.log(
        `SYNC:${shortAddr(
            wallet.address()
        )} ${type} balance: ${balance} ${token}`
    );
}

async function logETHBalance(
    wallet: zksync.Wallet,
    token: zksync.types.TokenLike
) {
    const balance = await wallet.getEthereumBalance(token);

    console.log(
        `ETH:${shortAddr(wallet.address())} balance: ${balance} ${token}`
    );
}

(async () => {
    console.log(`Starting test on the ${network} network`);

    const ethersProvider = new ethers.providers.JsonRpcProvider(WEB3_URL);
    const syncProvider = await zksync.getDefaultProvider(network);
    const ethProxy = new zksync.ETHProxy(
        ethersProvider,
        syncProvider.contractAddress
    );

    console.log("Contract address: ", syncProvider.contractAddress);

    const ethWallet = ethers.Wallet.fromMnemonic(
        MNEMONIC,
        "m/44'/60'/0'/0/0"
    ).connect(ethersProvider);
    const syncWallet = await zksync.Wallet.fromEthSigner(
        ethWallet,
        syncProvider
    );

    const ethWallet2 = ethers.Wallet.fromMnemonic(
        MNEMONIC,
        "m/44'/60'/0'/0/1"
    ).connect(ethersProvider);
    const syncWallet2 = await zksync.Wallet.fromEthSigner(
        ethWallet2,
        syncProvider
    );

    const depositAmount = "0.0017";
    const depositToken = TOKEN;
    console.log("==================================");
    console.log(
        `Deposit: ${depositAmount} ${depositToken}: ETH:${shortAddr(
            ethWallet.address
        )} -> SYNC:${shortAddr(syncWallet.address())}`
    );
    await logETHBalance(syncWallet, depositToken);
    await logSyncBalance(syncWallet, depositToken, "verified");

    const depositHandle = await zksync.depositFromETH({
        depositFrom: ethWallet,
        depositTo: syncWallet,
        token: depositToken,
        amount: utils.parseEther(depositAmount)
    });
    const depositReceipt = await depositHandle.awaitReceipt();
    console.log("Deposit committed, block:", depositReceipt.block.blockNumber);

    await logETHBalance(syncWallet, depositToken);
    await logSyncBalance(syncWallet, depositToken, "committed");

    if (!(await syncWallet.isCurrentPubkeySet())) {
        console.log("==================================");
        console.log(
            "Unlocking account with onchain tx: ",
            syncWallet.address()
        );
        try {
            await (await syncWallet.authChangePubkey()).wait();
        } catch (e) {}
        const unlockAccountHandle = await syncWallet.setCurrentPubkeyWithZksyncTx(
            "committed",
            true
        );
        await unlockAccountHandle.awaitReceipt();
        console.log("Account unlocked");
    } else {
        console.log("Account: ", syncWallet.address(), "is unlocked");
    }
    console.log("==================================");
    console.log(
        `Transfer: ${depositAmount} ${depositToken}: SYNC:${shortAddr(
            syncWallet.address()
        )} -> SYNC:${shortAddr(syncWallet2.address())}`
    );
    await logSyncBalance(syncWallet, depositToken, "committed");
    await logSyncBalance(syncWallet2, depositToken, "committed");

    const transferHandle = await syncWallet.syncTransfer({
        to: syncWallet2.address(),
        token: depositToken,
        amount: utils.parseEther(depositAmount),
        fee: 0
    });
    const transferReceipt = await transferHandle.awaitReceipt();
    console.log(
        "Transfer committed, block:",
        transferReceipt.block.blockNumber
    );

    await logSyncBalance(syncWallet, depositToken, "committed");
    await logSyncBalance(syncWallet2, depositToken, "committed");

    if (!(await syncWallet2.isCurrentPubkeySet())) {
        console.log("==================================");
        console.log(
            "Unlocking account with offchain tx: ",
            syncWallet2.address()
        );
        const unlockAccount2Handle = await syncWallet2.setCurrentPubkeyWithZksyncTx();
        await unlockAccount2Handle.awaitReceipt();
        console.log("Account unlocked");
    } else {
        console.log("Account: ", syncWallet2.address(), "is unlocked");
    }

    const withdrawAmount = formatEther(parseEther(depositAmount).div(2));
    console.log("==================================");
    console.log(
        `Withdraw: ${withdrawAmount} ${depositToken}: SYNC:${shortAddr(
            syncWallet2.address()
        )} -> ETH:${shortAddr(ethWallet2.address)}`
    );
    await logSyncBalance(syncWallet2, depositToken);
    await logETHBalance(syncWallet2, depositToken);

    const withdrawHandle = await syncWallet2.withdrawTo({
        ethAddress: ethWallet2.address,
        token: depositToken,
        amount: ethers.utils.parseEther(withdrawAmount),
        fee: 0
    });
    const withdrawReceipt = await withdrawHandle.awaitReceipt();
    console.log("Withdraw verified, block", withdrawReceipt.block.blockNumber);

    await logSyncBalance(syncWallet2, depositToken, "verified");
    await logETHBalance(syncWallet2, depositToken);

    console.log("==================================");
    await logSyncBalance(syncWallet2, depositToken, "committed");

    console.log(`FullExit of ${TOKEN} from: ${syncWallet2.address()}`);
    const fullExitHandle = await emergencyWithdraw({
        withdrawFrom: syncWallet2,
        token: TOKEN
    });
    const fullExitReceipt = await fullExitHandle.awaitReceipt();
    console.log(
        "Full exit committed, block",
        fullExitReceipt.block.blockNumber
    );

    await logSyncBalance(syncWallet2, depositToken, "committed");

    await syncProvider.disconnect();
})();
