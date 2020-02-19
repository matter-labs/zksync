import { AbstractJSONRPCTransport } from "./transport";
import { utils, ethers } from "ethers";
import { AccountState, Address, TokenLike, TransactionReceipt, PriorityOperationReceipt, ContractAddress, Tokens, TokenAddress } from "./types";
import { TokenSet } from "./utils";
export declare function getDefaultProvider(network: "localhost" | "testnet", transport?: "WS" | "HTTP"): Promise<Provider>;
export declare class Provider {
    transport: AbstractJSONRPCTransport;
    contractAddress: ContractAddress;
    tokenSet: TokenSet;
    private constructor();
    static newWebsocketProvider(address: string): Promise<Provider>;
    static newHttpProvider(address?: string): Promise<Provider>;
    submitTx(tx: any): Promise<string>;
    getContractAddress(): Promise<ContractAddress>;
    getTokens(): Promise<Tokens>;
    getState(address: Address): Promise<AccountState>;
    getTxReceipt(txHash: string): Promise<TransactionReceipt>;
    getPriorityOpStatus(serialId: number): Promise<PriorityOperationReceipt>;
    notifyPriorityOp(serialId: number, action: "COMMIT" | "VERIFY"): Promise<PriorityOperationReceipt>;
    notifyTransaction(hash: string, action: "COMMIT" | "VERIFY"): Promise<TransactionReceipt>;
    disconnect(): Promise<any>;
}
export declare class ETHProxy {
    private ethersProvider;
    contractAddress: ContractAddress;
    private governanceContract;
    private mainContract;
    constructor(ethersProvider: ethers.providers.Provider, contractAddress: ContractAddress);
    resolveTokenId(token: TokenAddress): Promise<number>;
    estimateDepositFeeInETHToken(token: TokenLike, gasPrice?: utils.BigNumber): Promise<utils.BigNumber>;
    estimateEmergencyWithdrawFeeInETHToken(gasPrice?: utils.BigNumber): Promise<utils.BigNumber>;
}
