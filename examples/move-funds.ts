import {
    depositFromETH,
    emergencyWithdraw,
    SyncWallet
} from "../src/syncWallet";
import { Contract, ethers, utils } from "ethers";
import { formatEther } from "ethers/utils";
import { ETHProxy, SyncProvider } from "../src/provider";
import { Token } from "../src/types";
import { IERC20_INTERFACE } from "../src/utils";

const WEB3_URL = process.env.WEB3_URL;
// Mnemonic for eth wallet.
const MNEMONIC = process.env.MNEMONIC;
const TOKEN = process.env.TEST_ERC20;

function shortAddr(address: string): string {
    return `${address.substr(0, 6)}`;
}

async function logSyncBalance(
    wallet: SyncWallet,
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
    } else {
        const erc20contract = new Contract(token, IERC20_INTERFACE, wallet);
        balance = formatEther(await erc20contract.balanceOf(wallet.address));
    }

    console.log(
        `ETH:${shortAddr(wallet.address)} balance: ${balance} ${token}`
    );
}

(async () => {
    const ethersProvider = new ethers.providers.JsonRpcProvider(WEB3_URL);
    const wsSidechainProvider = await SyncProvider.newHttpProvider();
    const ethProxy = new ETHProxy(
        ethersProvider,
        wsSidechainProvider.contractAddress
    );

    console.log("Contract address: ", wsSidechainProvider.contractAddress);

    const ethWallet = ethers.Wallet.fromMnemonic(
        MNEMONIC,
        "m/44'/60'/0'/0/1"
    ).connect(ethersProvider);
    const wallet = await SyncWallet.fromEthWallet(
        ethWallet,
        wsSidechainProvider,
        ethProxy
    );

    const ethWallet2 = ethers.Wallet.fromMnemonic(
        MNEMONIC,
        "m/44'/60'/0'/0/2"
    ).connect(ethersProvider);
    const wallet2 = await SyncWallet.fromEthWallet(
        ethWallet2,
        wsSidechainProvider,
        ethProxy
    );

    const depositAmount = "17.0";
    const depositToken = TOKEN;
    console.log("==================================");
    console.log(
        `Deposit: ${depositAmount} ${depositToken}: ETH:${shortAddr(
            ethWallet.address
        )} -> SYNC:${shortAddr(wallet.address())}`
    );
    await logETHBalance(ethWallet, depositToken);
    await logSyncBalance(wallet, depositToken, "verified");

    const depositHandle = await depositFromETH(
        ethWallet,
        wallet,
        depositToken,
        utils.parseEther(depositAmount),
        utils.parseEther("0.1")
    );
    await depositHandle.waitCommit();
    console.log("Deposit commited");

    await logETHBalance(ethWallet, depositToken);
    await logSyncBalance(wallet, depositToken, "commited");

    console.log("==================================");
    console.log(
        `Transfer: ${depositAmount} ${depositToken}: SYNC:${shortAddr(
            wallet.address()
        )} -> SYNC:${shortAddr(wallet2.address())}`
    );
    await logSyncBalance(wallet, depositToken, "commited");
    await logSyncBalance(wallet2, depositToken, "commited");

    const transferHandle = await wallet.syncTransfer(
        wallet2.address(),
        depositToken,
        utils.parseEther(depositAmount),
        0
    );
    await transferHandle.waitCommit();
    console.log("Transfer commited");

    await logSyncBalance(wallet, depositToken, "commited");
    await logSyncBalance(wallet2, depositToken, "commited");

    console.log("==================================");
    console.log(
        `Withdraw: ${depositAmount} ${depositToken}: SYNC:${shortAddr(
            wallet.address()
        )} -> ETH:${shortAddr(ethWallet2.address)}`
    );
    await logSyncBalance(wallet2, depositToken);
    await logETHBalance(ethWallet2, depositToken);

    const withdrawHandle = await wallet2.withdrawTo(
        ethWallet2.address,
        depositToken,
        ethers.utils.parseEther(depositAmount),
        0
    );
    await withdrawHandle.waitVerify();
    console.log("Withdraw commited");

    await logSyncBalance(wallet2, depositToken, "verified");
    await logETHBalance(ethWallet2, depositToken);

    await wsSidechainProvider.disconnect();
})();
