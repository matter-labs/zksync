import { AbstractTransport, HTTPTransport, WSTransport } from "./transport";
import { utils } from "ethers";
import {
  Address,
  CloseTx,
  SidechainAccountState,
  SidechainInfo, Token,
  TokenLike,
  TransferTx,
  WithdrawTx
} from "./types";

export class SyncProvider {
  contractAddress: string;

  static async newWebsocketProvider(
    address: string = "ws://127.0.0.1:3031"
  ): Promise<SyncProvider> {
    const transport = await WSTransport.connect(address);
    const provider = new SyncProvider(transport);
    await provider.updateSidechainInfo();
    return provider;
  }

  static async newHttpProvider(
    address: string = "http://127.0.0.1:3030"
  ): Promise<SyncProvider> {
    const transport = new HTTPTransport(address);
    const provider = new SyncProvider(transport);
    await provider.updateSidechainInfo();
    return provider;
  }

  private constructor(public transport: AbstractTransport) {}

  private async updateSidechainInfo() {
    this.sideChainInfo = await this.getSidechainInfo();
  }

  static prepareTransferTxForApi(tx: TransferTx, signature) {
    const req: any = tx;
    req.type = "Transfer";
    req.from = tx.from;
    req.to = tx.to;
    req.amount = utils.bigNumberify(tx.amount).toString();
    req.fee = utils.bigNumberify(tx.fee).toString();
    req.signature = signature;
    return req;
  }

  static prepareWithdrawTxForApi(tx: WithdrawTx, signature) {
    const req: any = tx;
    req.type = "Withdraw";
    req.account = tx.account;
    req.amount = utils.bigNumberify(tx.amount).toString();
    req.fee = utils.bigNumberify(tx.fee).toString();
    req.signature = signature;
    return req;
  }

  static prepareCloseRequestForApi(tx: CloseTx, signature) {
    const req: any = tx;
    req.type = "Close";
    req.account = tx.account;
    req.signature = signature;
    return req;
  }

  // return transaction hash (e.g. 0xdead..beef)
  async submitTx(tx: any): Promise<string> {
    return await this.transport.request("tx_submit", [tx]);
  }

  async getSidechainInfo(): Promise<SidechainInfo> {
    return await this.transport.request("chain_info", null);
  }

  async getState(address: Address): Promise<SidechainAccountState> {
    return await this.transport.request("account_info", [address]);
  }

  // get transaction status by its hash (e.g. 0xdead..beef)
  async getTxReceipt(txHash: string) {
    return await this.transport.request("tx_info", [txHash]);
  }

  async getPriorityOpStatus(serialId: number) {
    return await this.transport.request("ethop_info", [serialId]);
  }

  async notifyPriorityOp(serialId: number, action: "COMMIT" | "VERIFY") {
    return await new Promise(resolve => {
      const sub = this.transport.subscribe(
        "ethop_subscribe",
        [serialId, action],
        "ethop_unsubscribe",
        resp => {
          sub.then(sub => sub.unsubscribe());
          resolve(resp);
        }
      );
    });
  }

  async notifyTransaction(hash: string, action: "COMMIT" | "VERIFY") {
    return await new Promise(resolve => {
      const sub = this.transport.subscribe(
        "tx_subscribe",
        [hash, action],
        "tx_unsubscribe",
        resp => {
          sub.then(sub => sub.unsubscribe());
          resolve(resp);
        }
      );
    });
  }

  async resolveTokenId(token: Token): Promise<number> {
      if (token == "ETH") {
        return 0;
      } else {
        throw new Error("not implemented");
      }
  }
}
