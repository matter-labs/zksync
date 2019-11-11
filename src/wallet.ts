import BN = require("bn.js");
import {
  privateKeyFromSeed,
  privateKeyToPublicKey,
  pubkeyToAddress,
  serializePointPacked,
  signTransactionBytes
} from "./crypto";
import { Contract, ContractTransaction, ethers, utils } from "ethers";
import { curve } from "elliptic";
import { SyncProvider } from "./provider";
import {SyncSigner} from "./signer";
import {SyncAccountState, SyncAddress, SyncWithdraw, Token} from "./types";

const IERC20ConractInterface = new utils.Interface(
  require("../abi/IERC20.json").interface
);
const sidechainMainContractInterface = new utils.Interface(
  require("../abi/SidechainMain.json").interface
);
const priorityQueueInterface = new utils.Interface(
  require("../abi/PriorityQueue.json").interface
);

export class Wallet {
  constructor(public signer: SyncSigner, public provider: SyncProvider) {}

  async syncTransfer(
    to: SyncAddress,
    token: Token,
    amount: utils.BigNumberish,
    fee: utils.BigNumberish,
    nonce: "commited" | number = "commited"
  ): Promise<TransactionHandle> {
    const tokenId = await this.provider.resolveTokenId(token);
    const transaxtionData = {
      to,
      tokenId,
      amount,
      fee,
      nonce: await this.getNonce(nonce)
    };
    const signedTransferTransaction = this.signer.signSyncTransfer(transaxtionData);

    const transactionHash = await this.provider.submitTx(signedTransferTransaction);
    return new TransactionHandle(signedTransferTransaction, transactionHash, this.provider);
  }

  async withdrawTo(
      ethAddress: string,
      token: Token,
      amount: utils.BigNumberish,
      fee: utils.BigNumberish,
      nonce: "commited" | number = "commited"
  ): Promise<TransactionHandle> {
    const tokenId = await this.provider.resolveTokenId(token);
    const transactionData = {
      ethAddress,
      tokenId,
      amount,
      fee,
      nonce: await this.getNonce(nonce)
    };
    const signedWithdrawTransaction = this.signer.signSyncWithdraw(transactionData);

    const submitResponse = await this.provider.submitTx(signedWithdrawTransaction);
    return new TransactionHandle(signedWithdrawTransaction, submitResponse, this.provider);
  }

  async close(nonce: "commited" | number = "commited"): Promise<TransactionHandle> {
    const signerdCloseTransaction = this.signer.signSyncCloseAccount({nonce: await this.getNonce()});

    const transactionHash = await this.provider.submitTx(signerdCloseTransaction);
    return new TransactionHandle(signerdCloseTransaction, transactionHash, this.provider);
  }

  async getNonce(nonce: "commited" | number = "commited"): Promise<number> {
    if (nonce == "commited") {
      return (await this.provider.getState(this.signer.address())).commited.nonce;
    } else if (typeof nonce == "number") {
      return nonce;
    }
  }

  address(): SyncAddress {
    return this.signer.address();
  }

  static async fromEthWallet(
    ethWallet: ethers.Signer,
    sidechainProvider: SyncProvider
  ) {
    const seedHex = (await ethWallet.signMessage("Matter login")).substr(2);
    const seed = Buffer.from(seedHex, "hex");
    let signer = SyncSigner.fromSeed(seed);
    return new Wallet(signer, sidechainProvider);
  }

  async getAccountState(): Promise<SyncAccountState> {
    return this.provider.getState(this.signer.address());
  }
}

export async function depositFromETH( depositFrom: ethers.Signer, depositTo: Wallet, token: Token, amount: utils.BigNumberish, maxFeeInETHCurrenty: utils.BigNumberish) {
   const mainSidechainContract = new Contract(
       depositTo.provider.contractAddress,
       sidechainMainContractInterface,
       depositFrom
   );

   let ethTransaction;

   if (token == "ETH") {
     ethTransaction = mainSidechainContract.depositETH(amount, depositTo.address(), {
       value: utils.bigNumberify(amount).add(maxFeeInETHCurrenty),
       gasLimit: utils.bigNumberify("200000")
     });
   } else {
     // ERC20 token deposit
     const erc20contract = new Contract(
         token,
         IERC20ConractInterface,
         depositFrom
     );
     const approveTx = await erc20contract.approve(
         depositTo.provider.contractAddress,
         amount
     );
     ethTransaction = await mainSidechainContract.depositERC20(
         token,
         amount,
         depositTo.address(),
         { gasLimit: utils.bigNumberify("250000"), value: maxFeeInETHCurrenty, nonce: (approveTx.nonce + 1) },
     );
   }

   return new DepositTransactionHandle(ethTransaction, depositTo.provider);
}

class DepositTransactionHandle {
  state: "Sent" | "Mined" | "Commited" | "Verified";
  priorityOpId?: utils.BigNumber;

  constructor(
    public ethTx: ContractTransaction,
    public sidechainProvider: SyncProvider
  ) {
    this.state = "Sent";
  }

  async waitTxMine() {
    if (this.state != "Sent") return;

    const txReceipt = await this.ethTx.wait();
    for (const log of txReceipt.logs) {
      const priorityQueueLog = priorityQueueInterface.parseLog(
        txReceipt.logs[0]
      );
      if (priorityQueueLog) {
        this.priorityOpId = priorityQueueLog.values.serialId;
      }
    }
    if (!this.priorityOpId) {
      throw new Error("Failed to parse tx logs");
    }

    this.state = "Mined";
  }

  async waitCommit() {
    await this.waitTxMine();
    if (this.state != "Mined") return;
    await this.sidechainProvider.notifyPriorityOp(
      this.priorityOpId.toNumber(),
      "COMMIT"
    );
    this.state = "Commited";
  }

  async waitVerify() {
    await this.waitCommit();
    if (this.state != "Commited") return;

    await this.sidechainProvider.notifyPriorityOp(
      this.priorityOpId.toNumber(),
      "VERIFY"
    );
    this.state = "Verified";
  }
}

class TransactionHandle {
  state: "Sent" | "Commited" | "Verified";

  constructor(
    public txData,
    public txHash: string,
    public sidechainProvider: SyncProvider
  ) {
    this.state = "Sent";
  }

  async waitCommit() {
    if (this.state !== "Sent") return;

    await this.sidechainProvider.notifyTransaction(this.txHash, "COMMIT");
    this.state = "Commited";
  }

  async waitVerify() {
    await this.waitCommit();
    await this.sidechainProvider.notifyTransaction(this.txHash, "VERIFY");
    this.state = "Verified";
  }
}


// async emergencyWithdraw(token: Token, nonce: "commited" | number = "commited") {
//   const tokenId = await this.provider.resolveTokenId(token);
//   const sidechainMainContract = new Contract(
//       this.provider.contractAddress,
//       sidechainMainContractInterface,
//       this.ethWallet
//   );
//   const nonceNumber = await this.getNonce(nonce);
//   const signature = this.walletKeys.signFullExit({
//     token: token.id,
//     eth_address: await this.ethWallet.getAddress(),
//     nonce: nonceNumber
//   });
//   const tx = await sidechainMainContract.fullExit(
//       serializePointPacked(this.walletKeys.publicKey),
//       token.address,
//       signature,
//       nonceNumber,
//       {
//         gasLimit: utils.bigNumberify("500000"),
//         value: utils.parseEther("0.02")
//       }
//   );
//   return tx.hash;
// }

// async getETHBalances(): Promise<ETHAccountState> {
//   const tokens = this.provider.sideChainInfo.tokens;
//   const onchainBalances = new Array<utils.BigNumber>(tokens.length);
//   const contractBalances = new Array<utils.BigNumber>(tokens.length);
//
//   const sidechainMainContract = new Contract(
//     this.provider.sideChainInfo.contract_address,
//     sidechainMainContractInterface,
//     this.ethWallet
//   );
//   const ethAddress = await this.ethWallet.getAddress();
//   for (const token of tokens) {
//     if (token.id == 0) {
//       onchainBalances[token.id] = await this.ethWallet.provider.getBalance(
//         ethAddress
//       );
//     } else {
//       const erc20token = new Contract(
//         token.address,
//         IERC20ConractInterface,
//         this.ethWallet
//       );
//       onchainBalances[token.id] = await erc20token.balanceOf(ethAddress);
//     }
//     contractBalances[
//       token.id
//     ] = await sidechainMainContract.balancesToWithdraw(ethAddress, token.id);
//   }
//
//   return { onchainBalances, contractBalances };
// }
