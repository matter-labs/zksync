import { AbstractTransport, HTTPTransport, WSTransport } from "./transport";
import { utils } from "ethers";
import {SyncAccountState, SyncAddress, Token} from "./types";

export class SyncProvider {
  contractAddress: string;
  private constructor(public transport: AbstractTransport) {}

  static async newWebsocketProvider(
    address: string = "ws://127.0.0.1:3031"
  ): Promise<SyncProvider> {
    const transport = await WSTransport.connect(address);
    const provider = new SyncProvider(transport);
    provider.contractAddress = await provider.getContractAddress();
    return provider;
  }

  static async newHttpProvider(
    address: string = "http://127.0.0.1:3030"
  ): Promise<SyncProvider> {
    const transport = new HTTPTransport(address);
    const provider = new SyncProvider(transport);
    provider.contractAddress = await provider.getContractAddress();
    return provider;
  }


  // return transaction hash (e.g. 0xdead..beef)
  async submitTx(tx: any): Promise<string> {
    return await this.transport.request("tx_submit", [tx]);
  }

  async getContractAddress(): Promise<string> {
    return await this.transport.request("contract_address", null);
  }

  async getState(address: SyncAddress): Promise<SyncAccountState> {
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
        throw new Error("unimplemented erc20");
      }
  }
}
