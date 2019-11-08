import BN = require("bn.js");
import {
  privateKeyFromSeed,
  privateKeyToPublicKey,
  pubkeyToAddress,
  serializePointPacked,
  signTransactionBytes
} from "./crypto";
import { Contract, ContractTransaction, ethers, utils } from "ethers";
import { packAmount, packFee } from "./utils";
import { curve } from "elliptic";
import {
  Address,
  CloseTx,
  DepositTx,
  ETHAccountState,
  FullExitReq,
  Nonce,
  SidechainAccountState, Token,
  TokenLike,
  TransferTx,
  WithdrawTx
} from "./types";
import { SidechainProvider } from "./provider";
import { WalletKeys } from "./signer";

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
  walletKeys: WalletKeys;

  constructor(seed: Buffer, public provider: SidechainProvider) {}

  async withdrawTo(
    ethAddress: string,
    tokenLike: TokenLike,
    amount: utils.BigNumberish,
    fee: utils.BigNumberish,
    nonce: Nonce = "commited"
  ): Promise<TransactionHandle> {
    const token = (await this.provider.resolveToken(tokenLike)).id;
    const tx = {
      account: this.walletKeys.address(),
      ethAddress,
      token,
      amount,
      fee,
      nonce: await this.getNonce(nonce)
    };
    const signature = this.walletKeys.signWithdraw(tx);
    const tx_req = SidechainProvider.prepareWithdrawTxForApi(tx, signature);

    const submitResponse = await this.provider.submitTx(tx_req);
    return new TransactionHandle(tx, submitResponse, this.provider);
  }

  async transfer(
    to: Address,
    token: Token,
    amount: utils.BigNumberish,
    fee: utils.BigNumberish,
    nonce: "commited" | number = "commited"
  ): Promise<TransactionHandle> {
    const tokenId = await this.provider.resolveToken(tokenLike);
    const signedTransfer = await this.walletKeys.sign(..);
    const tx = {
      from: this.address,
      to,
      token: token.id,
      amount,
      fee,
      nonce: await this.getNonce(nonce)
    };
    const signature = this.walletKeys.signTransfer(tx);
    const tx_req = SidechainProvider.prepareTransferTxForApi(tx, signature);

    const submitResponse = await this.provider.submitTx(tx_req);
    return new TransactionHandle(tx, submitResponse, this.provider);
  }

  async emergencyWithdraw(tokenLike: TokenLike, nonce: Nonce = "commited") {
    const token = await this.provider.resolveToken(tokenLike);
    const sidechainMainContract = new Contract(
      this.provider.sideChainInfo.contract_address,
      sidechainMainContractInterface,
      this.ethWallet
    );
    const nonceNumber = await this.getNonce(nonce);
    const signature = this.walletKeys.signFullExit({
      token: token.id,
      eth_address: await this.ethWallet.getAddress(),
      nonce: nonceNumber
    });
    const tx = await sidechainMainContract.fullExit(
      serializePointPacked(this.walletKeys.publicKey),
      token.address,
      signature,
      nonceNumber,
      {
        gasLimit: utils.bigNumberify("500000"),
        value: utils.parseEther("0.02")
      }
    );
    return tx.hash;
  }

  async close(): Promise<TransactionHandle> {
    const tx = {
      account: this.address,
      nonce: await this.getNonce()
    };

    const signature = this.walletKeys.signClose(tx);
    const tx_req = SidechainProvider.prepareCloseRequestForApi(tx, signature);

    const submitResponse = await this.provider.submitTx(tx_req);
    return new TransactionHandle(tx, submitResponse, this.provider);
  }

  async getNonce(nonce: Nonce = "commited"): Promise<number> {
    if (nonce == "commited") {
      return (await this.provider.getState(this.address)).commited.nonce;
    } else if (typeof nonce == "number") {
      return nonce;
    }
  }

  static async fromEthWallet(
    ethWallet: ethers.Signer,
    sidechainProvider: SidechainProvider
  ) {
    const seed = (await ethWallet.signMessage("Matter login")).substr(2);
    return new Wallet(Buffer.from(seed, "hex"), sidechainProvider, ethWallet);
  }

  async getETHBalances(): Promise<ETHAccountState> {
    const tokens = this.provider.sideChainInfo.tokens;
    const onchainBalances = new Array<utils.BigNumber>(tokens.length);
    const contractBalances = new Array<utils.BigNumber>(tokens.length);

    const sidechainMainContract = new Contract(
      this.provider.sideChainInfo.contract_address,
      sidechainMainContractInterface,
      this.ethWallet
    );
    const ethAddress = await this.ethWallet.getAddress();
    for (const token of tokens) {
      if (token.id == 0) {
        onchainBalances[token.id] = await this.ethWallet.provider.getBalance(
          ethAddress
        );
      } else {
        const erc20token = new Contract(
          token.address,
          IERC20ConractInterface,
          this.ethWallet
        );
        onchainBalances[token.id] = await erc20token.balanceOf(ethAddress);
      }
      contractBalances[
        token.id
      ] = await sidechainMainContract.balancesToWithdraw(ethAddress, token.id);
    }

    return { onchainBalances, contractBalances };
  }

  async getAccountState(): Promise<SidechainAccountState> {
    return this.provider.getState(this.address);
  }
}

class DepositTransactionHandle {
  state: "Sent" | "Mined" | "Commited" | "Verified";
  priorityOpId?: utils.BigNumber;

  constructor(
    public ethTx: ContractTransaction,
    public depositTx: DepositTx,
    public sidechainProvider: SidechainProvider
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
    public sidechainProvider: SidechainProvider
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

// protected async depositETH(
//     amount: utils.BigNumberish,
//     maxFee: utils.BigNumberish
// ) {
//   const mainSidechainContract = new Contract(
//       this.provider.sideChainInfo.contract_address,
//       sidechainMainContractInterface,
//       this.ethWallet
//   );
//   return await mainSidechainContract.depositETH(amount, this.address, {
//     value: utils.bigNumberify(amount).add(maxFee),
//     gasLimit: utils.bigNumberify("200000")
//   });
// }
//
// protected async approveERC20(
//     tokenLike: TokenLike,
//     amount: utils.BigNumberish,
//     options?: Object
// ) {
//   const token = await this.provider.resolveToken(tokenLike);
//   const erc20contract = new Contract(
//       token.address,
//       IERC20ConractInterface,
//       this.ethWallet
//   );
//   return await erc20contract.approve(
//       this.provider.sideChainInfo.contract_address,
//       amount,
//       options
//   );
// }
//
// protected async depositApprovedERC20(
//     tokenLike: TokenLike,
//     amount: utils.BigNumberish,
//     maxEthFee: utils.BigNumberish,
//     options?: Object
// ) {
//   const token = await this.provider.resolveToken(tokenLike);
//   const mainSidechainContract = new Contract(
//       this.provider.sideChainInfo.contract_address,
//       sidechainMainContractInterface,
//       this.ethWallet
//   );
//   return await mainSidechainContract.depositERC20(
//       token.address,
//       amount,
//       this.address,
//       Object.assign(
//           { gasLimit: utils.bigNumberify("250000"), value: maxEthFee },
//           options
//       )
//   );
// }

// async deposit(
//     tokenLike: TokenLike,
//     amount: utils.BigNumberish,
//     maxEthFee: utils.BigNumberish
// ) {
//   const token = await this.provider.resolveToken(tokenLike);
//   let contractTx;
//   if (token.id === 0) {
//     contractTx = await this.depositETH(amount, maxEthFee);
//   } else {
//     await this.approveERC20(token, amount);
//     contractTx = await this.depositApprovedERC20(token, amount, maxEthFee);
//   }
//   return new DepositTransactionHandle(
//       contractTx,
//       { to: this.address, amount, token },
//       this.provider
//   );
// }
