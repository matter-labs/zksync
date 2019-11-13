import BN = require("bn.js");
import { depositFromETH, Wallet } from "../src/wallet";
import { ethers, utils } from "ethers";
import { bigNumberify, formatEther, parseEther } from "ethers/utils";
import { SyncProvider } from "../src/provider";
import { WSTransport } from "../src/transport";
import { Token } from "../src/types";

function shortAddr(address: string): string {
    return `${address.substr(0, 6)}`;
}

async function logSyncBalance(
    wallet: Wallet,
    token: Token,
    type: "commited" | "verified" = "commited"
) {
    const balance = formatEther(await wallet.getBalance(token, type));
    console.log(
        `SYNC:${shortAddr(
            wallet.address()
        )} ${type} balance: ${balance} ${token}`
    );
}
async function logETHBalance(wallet: ethers.Wallet, token: Token) {
    let balance;
    if (token == "ETH") {
        balance = formatEther(await wallet.getBalance());
    }

    console.log(
        `ETH:${shortAddr(wallet.address)} balance: ${balance} ${token}`
    );
}

(async () => {
    const ethersProvider = new ethers.providers.JsonRpcProvider(
        process.env.WEB3_URL
    );
    const wsSidechainProvider = await SyncProvider.newWebsocketProvider();

    console.log("Contract address: ", wsSidechainProvider.contractAddress);

    const ethWallet = ethers.Wallet.fromMnemonic(
        process.env.MNEMONIC,
        "m/44'/60'/0'/0/1"
    ).connect(ethersProvider);
    const wallet = await Wallet.fromEthWallet(ethWallet, wsSidechainProvider);

    const ethWallet2 = ethers.Wallet.fromMnemonic(
        process.env.MNEMONIC,
        "m/44'/60'/0'/0/2"
    ).connect(ethersProvider);
    const wallet2 = await Wallet.fromEthWallet(ethWallet2, wsSidechainProvider);

    const depositAmount = "0.1";
    const depositToken = "ETH";
    console.log("==================================");
    console.log(
        `Deposit: ${depositAmount} ${depositToken}: ETH:${shortAddr(
            ethWallet.address
        )} -> SYNC:${shortAddr(wallet.address())}`
    );
    await logETHBalance(ethWallet, "ETH");
    await logSyncBalance(wallet, "ETH", "verified");

    const depositHandle = await depositFromETH(
        ethWallet,
        wallet,
        "ETH",
        utils.parseEther(depositAmount),
        utils.parseEther("0.1")
    );
    await depositHandle.waitCommit();
    console.log("Deposit commited");

    await logETHBalance(ethWallet, "ETH");
    await logSyncBalance(wallet, "ETH", "commited");

    console.log("==================================");
    console.log(
        `Transfer: ${depositAmount} ${depositToken}: SYNC:${shortAddr(
            wallet.address()
        )} -> SYNC:${shortAddr(wallet2.address())}`
    );
    await logSyncBalance(wallet, "ETH", "commited");
    await logSyncBalance(wallet2, "ETH", "commited");

    const transferHandle = await wallet.syncTransfer(
        wallet2.address(),
        "ETH",
        utils.parseEther(depositAmount),
        0
    );
    await transferHandle.waitCommit();
    console.log("Transfer commited");

    await logSyncBalance(wallet, "ETH", "commited");
    await logSyncBalance(wallet2, "ETH", "commited");

    console.log("==================================");
    console.log(
        `Withdraw: ${depositAmount} ${depositToken}: SYNC:${shortAddr(
            wallet.address()
        )} -> ETH:${shortAddr(ethWallet2.address)}`
    );
    await logSyncBalance(wallet2, "ETH");
    await logETHBalance(ethWallet2, "ETH");

    const withdrawHandle = await wallet2.withdrawTo(
        ethWallet2.address,
        "ETH",
        ethers.utils.parseEther(depositAmount),
        0
    );
    await withdrawHandle.waitVerify();
    console.log("Withdraw commited");

    await logSyncBalance(wallet2, "ETH", "verified");
    await logETHBalance(ethWallet2, "ETH");

    await (wsSidechainProvider.transport as WSTransport).ws.close();
})();
