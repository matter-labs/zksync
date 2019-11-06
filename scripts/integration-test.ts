import BN = require("bn.js");
import { Wallet } from "../src/wallet";
import { ethers } from "ethers";
import { bigNumberify, formatEther, parseEther } from "ethers/utils";
import { SidechainProvider } from "../src/provider";
import { WSTransport } from "../src/transport";

function sleep(ms) {
  return new Promise(resolve => {
    setTimeout(resolve, ms);
  });
}

async function main() {
  const ethersProvider = new ethers.providers.JsonRpcProvider(
    process.env.WEB3_URL
  );
  const wsSidechainProvider = await SidechainProvider.newWebsocketProvider();

  console.log(
    "Contract address: ",
    wsSidechainProvider.sideChainInfo.contract_address
  );
  console.log("Supported tokens");
  for (const token of wsSidechainProvider.sideChainInfo.tokens) {
    const symbol = token.symbol ? token.symbol : "Ø";
    const address = token.id !== 0 ? token.address : "Ø";
    console.log(
      `Token id: ${token.id}, symbol: ${symbol}, address: ${address}`
    );
  }

  const ethTokenId = (await wsSidechainProvider.resolveToken("ETH")).id;

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

  // fund wallet 2
  const fundTx = await ethWallet.sendTransaction({
    to: ethWallet2.address,
    value: ethers.utils.parseEther("1")
  });
  await fundTx.wait();

  const depositAmount = "0.1";
  console.log("==================================");
  console.log("Wallet 1 deposit: ", depositAmount);
  console.log(
    "Wallet 1 ETH onchain balance",
    formatEther(
      (await wallet.getETHBalances()).onchainBalances[ethTokenId].toString()
    )
  );
  console.log(
    "Wallet 1 ETH sidechain balance",
    formatEther((await wallet.getAccountState()).commited.balances[ethTokenId])
  );
  const depHandle = await wallet.deposit(
    "ETH",
    parseEther(depositAmount),
    parseEther("0.2")
  );
  await depHandle.waitCommit();
  console.log("Deposit commited");
  console.log(
    "Wallet 1 ETH onchain balance",
    formatEther(
      (await wallet.getETHBalances()).onchainBalances[ethTokenId].toString()
    )
  );
  console.log(
    "Wallet 1 ETH sidechain balance",
    formatEther((await wallet.getAccountState()).commited.balances[ethTokenId])
  );

  console.log("==================================");
  console.log("Transfer offchain Wallet 1 -> Wallet 2: ", depositAmount);
  console.log(
    "Wallet 1 ETH sidechain balance",
    formatEther((await wallet.getAccountState()).commited.balances[ethTokenId])
  );
  console.log(
    "Wallet 2 ETH sidechain balance",
    formatEther((await wallet2.getAccountState()).commited.balances[ethTokenId])
  );
  const transferHandle = await wallet.transfer(
    wallet2.address,
    "ETH",
    ethers.utils.parseEther(depositAmount),
    0
  );
  await transferHandle.waitCommit();
  console.log("Transfer commited");
  console.log(
    "Wallet 1 ETH sidechain balance",
    formatEther((await wallet.getAccountState()).commited.balances[ethTokenId])
  );
  console.log(
    "Wallet 2 ETH sidechain balance",
    formatEther((await wallet2.getAccountState()).commited.balances[ethTokenId])
  );

  console.log("==================================");
  console.log("Wallet 2 withdraw ETH to contract");
  console.log(
    "Wallet 2 ETH sidechain balance",
    formatEther((await wallet2.getAccountState()).commited.balances[ethTokenId])
  );
  console.log(
    "Wallet 2 ETH balance on contract",
    formatEther(
      (await wallet2.getETHBalances()).contractBalances[ethTokenId].toString()
    )
  );
  const withdrawOffchainHandle = await wallet2.withdrawFromSidechainToContract(
    "ETH",
    ethers.utils.parseEther(depositAmount),
    0
  );
  await withdrawOffchainHandle.waitVerify();
  console.log("Withdraw verified");
  console.log(
    "Wallet 2 ETH sidechain balance",
    formatEther((await wallet2.getAccountState()).commited.balances[ethTokenId])
  );
  console.log(
    "Wallet 2 ETH balance on contract",
    formatEther(
      (await wallet2.getETHBalances()).contractBalances[ethTokenId].toString()
    )
  );

  console.log("==================================");
  console.log(
    "Wallet 2 Transfer ETH from contract to balance: ",
    depositAmount
  );
  console.log(
    "Wallet 2 ETH balance",
    formatEther(
      (await wallet2.getETHBalances()).onchainBalances[ethTokenId].toString()
    )
  );
  console.log(
    "Wallet 2 ETH balance on contract",
    formatEther(
      (await wallet2.getETHBalances()).contractBalances[ethTokenId].toString()
    )
  );
  const onchainWithdrawHandle = await wallet2.withdrawFromContractToETHAddress(
    "ETH",
    ethers.utils.parseEther(depositAmount)
  );
  await onchainWithdrawHandle.wait();
  console.log(`Onchain withdraw successful ${onchainWithdrawHandle.hash}`);
  console.log(
    "Wallet 2 ETH balance",
    formatEther(
      (await wallet2.getETHBalances()).onchainBalances[ethTokenId].toString()
    )
  );
  console.log(
    "Wallet 2 ETH balance on contract",
    formatEther(
      (await wallet2.getETHBalances()).contractBalances[ethTokenId].toString()
    )
  );

  await (wsSidechainProvider.transport as WSTransport).ws.close();
}

main();
