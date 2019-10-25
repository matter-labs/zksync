import BN = require('bn.js');
import Axios from 'axios';
import {
    privateKeyFromSeed,
    privateKeyToPublicKey,
    pubkeyToAddress, serializePointPacked, signTransactionBytes,
} from './crypto';
import {Contract, ContractTransaction, ethers, utils} from 'ethers';
import {packAmount, packFee} from "./utils";
import {curve} from "elliptic";
import EventSource from 'eventsource';

const IERC20ConractInterface = new utils.Interface(require("../abi/IERC20.json").interface);
const franklinContractInterface = new utils.Interface(require("../abi/Franklin.json").interface);
const priorityQueueInterface = new utils.Interface(require("../abi/PriorityQueue.json").interface);

export type Address = string;

export interface Token {
    id: number,
    address: string,
    symbol?: string,
}
// token, symbol/eth erc20 contract address, token id
export type TokenLike = Token | string | number;
export type Nonce= number | "commited" | "pending";

export interface FranklinAccountBalanceState {
    address: Address,
    nonce: number,
    balances: any[],
}

export interface FranklinAccountState {
    id?: number,
    commited: FranklinAccountBalanceState,
    verified: FranklinAccountBalanceState,
    pending_txs: any[],
}

interface ETHAccountState {
    onchainBalances: utils.BigNumber[],
    contractBalances: utils.BigNumber[],
}

export class FranklinProvider {
    cachedTokens: [Token];
    constructor(public providerAddress: string = 'http://127.0.0.1:3000', public contractAddress: string = process.env.CONTRACT_ADDR){}

    static prepareTransferRequestForNode(tx: TransferTx, signature) {
        let req: any = tx;
        req.type = "Transfer";
        req.from = tx.from;
        req.to = tx.to;
        req.amount = utils.bigNumberify(tx.amount).toString();
        req.fee = utils.bigNumberify(tx.fee).toString();
        req.signature = signature;
        return req;
    }

    static prepareWithdrawRequestForNode(tx: WithdrawTx, signature) {
        let req: any = tx;
        req.type = "Withdraw";
        req.account = tx.account;
        req.amount = utils.bigNumberify(tx.amount).toString();
        req.fee = utils.bigNumberify(tx.fee).toString();
        req.signature = signature;
        return req;
    }

    static prepareCloseRequestForNode(tx: CloseTx, signature) {
        let req: any = tx;
        req.type = "Close";
        req.account = tx.account;
        req.signature = signature;
        return req;
    }

    // TODO: reconsider when wallet refactor.
    private static async axiosRequest(promise) {
        promise = promise
            .then(reps => reps.data)
            .catch(error => { 
                throw new Error(error.response ? error.response.data.message : 'Error: Network Error');
            });
        return await promise;
    }
    
    async getPriorityOpStatus(opId: number) {
        return FranklinProvider.axiosRequest(
            Axios.get(this.providerAddress + '/api/v0.1/priority_op/' + opId));
    }
    
    async notifyPriorityOp(opId: number, action: "commit" | "verify") {
        return FranklinProvider.axiosRequest(
            Axios.get(this.providerAddress + `/api/v0.1/priority_op_notify/${action}/${opId}`));
    }
    
    async notifyTransaction(hash: string, action: "commit" | "verify") {
        return FranklinProvider.axiosRequest(
            Axios.get(this.providerAddress + `/api/v0.1/tx_notify/${action}/${hash}`));
    }
    
    async getBlockStatus(block: number) {
        return FranklinProvider.axiosRequest(
            Axios.get(this.providerAddress + '/api/v0.1/search?query=' + block));
    }

    getAccountUpdates(address: Address, action: "commit" | "verify"): EventSource {
        console.log("curl " + this.providerAddress + `/api/v0.1/account_updates/${action}/${address}`);
        return new EventSource(this.providerAddress + `/api/v0.1/account_updates/${action}/${address}`);
    }

    async resolveToken(token: TokenLike): Promise<Token> {
        function findToken(tokens: [Token]): Token {
            if(typeof(token) == "string") {
                let resolvedToken = tokens.find( (t, idx, arr) => {return t.symbol == token || t.address == token});
                if (resolvedToken) {
                    return resolvedToken;
                } else {
                    throw new Error("Token address or symbol not found");
                }
            } else if (typeof(token) == "number") {
                let resolvedToken = tokens.find( (t, idx, arr) => {return t.id == token});
                if (resolvedToken) {
                    return resolvedToken;
                } else {
                    throw Error("Token id not found");
                }
            } else {
                return token;
            }
        }

        // search cached tokens

        try {
            return findToken(this.cachedTokens);
        } catch (e) {
            this.cachedTokens = await this.getTokens();
            return findToken(this.cachedTokens);
        }
    }
    
    async submitTx(tx) {
        return await FranklinProvider.axiosRequest(
            Axios.post(this.providerAddress + '/api/v0.1/submit_tx', tx));
    }
    
    async getTokens() {
        return await FranklinProvider.axiosRequest(
            Axios.get(this.providerAddress + '/api/v0.1/tokens'));
    }
    
    async getTransactionsHistory(address: Address, offset: number, limit: number) {
        const link = `${this.providerAddress}/api/v0.1/account/0x${address.toString("hex")}/history/${offset}/${limit}`;
        console.log(`In wallet, we request ${link}`);
        return await FranklinProvider.axiosRequest(
            Axios.get(link));
    }
    
    async getState(address: Address): Promise<FranklinAccountState> {
        return await FranklinProvider.axiosRequest(
            Axios.get(this.providerAddress + '/api/v0.1/account/' + `0x${address.toString("hex")}`));
    }
    
    async getTxReceipt(tx_hash) {
        return await FranklinProvider.axiosRequest(
            Axios.get(this.providerAddress + '/api/v0.1/transactions/' + tx_hash));
    }
    
    async getPriorityOpReceipt(pq_id) {
        return await FranklinProvider.axiosRequest(
            Axios.get(`${this.providerAddress}/api/v0.1/priority_operations/${pq_id}/`));
    }
}    
export interface Token {
    id: number,
    address: string,
    symbol?: string,
}
        // search cached tokens

        try {
            return findToken(this.cachedTokens);
        } catch (e) {
            this.cachedTokens = await this.getTokens();
            return findToken(this.cachedTokens);
        }
    }
}

export interface DepositTx {
    to: Address,
    amount: utils.BigNumberish,
    token: Token,
}

export interface TransferTx {
    from: Address,
    to: Address,
    token: number,
    amount: utils.BigNumberish,
    fee: utils.BigNumberish,
    nonce: number,
}

export interface WithdrawTx {
    account: Address,
    eth_address: string,
    token: number,
    amount: utils.BigNumberish,
    fee: utils.BigNumberish,
    nonce: number,
}

export interface CloseTx {
    account: Address,
    nonce: number,
}

export interface FullExitReq {
    token: number,
    eth_address: string
    nonce: number,
}


// Franklin or eth address
function serializeAddress(address: Address | string): Buffer {
    return Buffer.from(address.substr(2), "hex");
}
function serializeTokenId(tokenId: number): Buffer {
    const buffer = Buffer.alloc(2);
    buffer.writeUInt16BE(tokenId, 0);
    return buffer;
}

function serializeAmountPacked(amount: utils.BigNumberish): Buffer {
    let bnAmount = new BN(utils.bigNumberify(amount).toString() );
    return packAmount(bnAmount);
}

function serializeAmountFull(amount: utils.BigNumberish): Buffer {
    let bnAmount = new BN(utils.bigNumberify(amount).toString() );
    return bnAmount.toArrayLike(Buffer, "be", 16);
}

function serializeFeePacked(fee: utils.BigNumberish): Buffer {
    let bnFee = new BN(utils.bigNumberify(fee).toString());
    return packFee(bnFee);
}

function serializeNonce(nonce: number): Buffer {
    let buff = Buffer.alloc(4);
    buff.writeUInt32BE(nonce, 0);
    return buff
}

export class WalletKeys {
    publicKey: curve.edwards.EdwardsPoint;

    constructor(public privateKey: BN) {
        this.publicKey = privateKeyToPublicKey(privateKey);
    }

    signTransfer(tx: TransferTx) {
        let type = Buffer.from([5]); // tx type
        let from = serializeAddress(tx.from);
        let to = serializeAddress(tx.to);
        let token = serializeTokenId(tx.token);
        let amount = serializeAmountPacked(tx.amount);
        let fee = serializeFeePacked(tx.fee);
        let nonce = serializeNonce(tx.nonce);
        let msg = Buffer.concat([type, from, to, token, amount, fee, nonce]);
        return signTransactionBytes(this.privateKey, msg);
    }

    signWithdraw(tx: WithdrawTx) {
        let type = Buffer.from([3]);
        let account = serializeAddress(tx.account);
        let eth_address = serializeAddress(tx.eth_address);
        let token = serializeTokenId(tx.token);
        let amount = serializeAmountFull(tx.amount);
        let fee = serializeFeePacked(tx.fee);
        let nonce = serializeNonce(tx.nonce);
        let msg = Buffer.concat([type, account, eth_address, token, amount, fee, nonce]);
        return signTransactionBytes(this.privateKey, msg);
    }

    signClose(tx: CloseTx) {
        let type = Buffer.from([4]);
        let account = serializeAddress(tx.account);
        let nonce = serializeNonce(tx.nonce);

        let msg = Buffer.concat([type, account, nonce]);
        return signTransactionBytes(this.privateKey, msg);
    }

    signFullExit(op: FullExitReq) {
        let type = Buffer.from([6]);
        let packed_pubkey = serializePointPacked(this.publicKey);
        let eth_address = serializeAddress(op.eth_address);
        let token = serializeTokenId(op.token);
        let nonce = serializeNonce(op.nonce);
        let msg = Buffer.concat([type, packed_pubkey, eth_address, token, nonce]);
        return Buffer.from(signTransactionBytes(this.privateKey, msg).sign, "hex");
    }
}

class DepositTransactionHandle {
    state: "Sent" | "Mined" | "Commited" | "Verified";
    priorityOpId?: utils.BigNumber;

    constructor(public ethTx: ContractTransaction, public depositTx: DepositTx, public franklinProvider: FranklinProvider) {
        this.state = "Sent";
    }

    async waitTxMine() {
        if (this.state != "Sent") return;

        let txReceipt = await this.ethTx.wait();

        let priorityOpIds = txReceipt.logs
            .map(log => priorityQueueInterface.parseLog(log))
            .filter(Boolean)
            .map(priorityQueueLog => priorityQueueLog.values.serialId);

        if (priorityOpIds.length == 0) {
            throw "Failed to parse tx logs";
        }

        this.priorityOpId = priorityOpIds.shift();
        
        this.state = "Mined"
    }

    async waitCommit() {
        await this.waitTxMine();
        if (this.state != "Mined") return;
        await this.franklinProvider.notifyPriorityOp(this.priorityOpId.toNumber(), "commit");
        this.state = "Commited";
    }

    async waitVerify() {
        await this.waitCommit();
        if (this.state != "Commited") return;

        await this.franklinProvider.notifyPriorityOp(this.priorityOpId.toNumber(), "verify");
        this.state = "Verified";
    }
}

class TransactionHandle {
    state: "Sent" | "Commited" | "Verified";

    constructor(public txData, public txHash: string, public franklinProvider: FranklinProvider) {
        this.state = "Sent";
    }

    async waitCommit() {
        if (this.state != "Sent") return;

        await this.franklinProvider.notifyTransaction(this.txHash, "commit");
        this.state = "Commited";
    }

    async waitVerify() {
        await this.waitCommit();
        await this.franklinProvider.notifyTransaction(this.txHash, "verify");
        this.state = "Verified"
    }
}

export class Wallet {
    address: Address;
    walletKeys: WalletKeys;

    franklinState: FranklinAccountState;
    ethState: ETHAccountState;
    pendingNonce: number;

    constructor(seed: Buffer, public provider: FranklinProvider, public ethWallet: ethers.Signer, public ethAddress: string) {
        let {privateKey} = privateKeyFromSeed(seed);
        this.walletKeys = new WalletKeys(privateKey);
        this.address = `0x${pubkeyToAddress(this.walletKeys.publicKey).toString("hex")}`;
    }

    async deposit(tokenLike: TokenLike, amount: utils.BigNumberish, fee: utils.BigNumberish = utils.parseEther("0.001")): Promise<DepositTransactionHandle> {
        let token = await this.provider.resolveToken(tokenLike);
        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractInterface, this.ethWallet);
        if (token.id == 0) {
            let totalAmount = utils.bigNumberify(amount).add(fee);
            const contractTx = await franklinDeployedContract.depositETH(amount, this.address, {value: totalAmount, gasLimit: utils.bigNumberify("200000")});
            return new DepositTransactionHandle(contractTx, {to: this.address, amount, token}, this.provider);
        } else {
            const erc20DeployedToken = new Contract(token.address, IERC20ConractInterface, this.ethWallet);
            await erc20DeployedToken.approve(franklinDeployedContract.address, amount);
            const contractTx = await franklinDeployedContract.depositERC20(erc20DeployedToken.address, amount, this.address,
                {gasLimit: utils.bigNumberify("300000"), value: fee});
            return new DepositTransactionHandle(contractTx, {to: this.address, amount, token}, this.provider);
        }
    }

    async widthdrawOnchain(tokenLike: TokenLike, amount: utils.BigNumberish): Promise<ContractTransaction> {
        let token = await this.provider.resolveToken(tokenLike);
        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractInterface, this.ethWallet);
        if (token.id == 0) {
            return await franklinDeployedContract.withdrawETH(amount, {gasLimit: 200000});
        } else {
            return await franklinDeployedContract.withdrawERC20(token.address, amount, {gasLimit: utils.bigNumberify("150000")});
        }
    }

    async widthdrawOffchain(tokenLike: TokenLike, amount: utils.BigNumberish, fee: utils.BigNumberish, nonce: Nonce = "commited"): Promise<TransactionHandle> {
        let token = await this.provider.resolveToken(tokenLike);
        let tx = {
            account: this.address,
            eth_address: await this.ethWallet.getAddress(),
            token: token.id,
            amount,
            fee,
            nonce: await this.getNonce(nonce),
        };
        let signature = this.walletKeys.signWithdraw(tx);
        let tx_req = FranklinProvider.prepareWithdrawRequestForNode(tx, signature);

        let submitResponse = await this.provider.submitTx(tx_req);
        return new TransactionHandle(tx, submitResponse.hash, this.provider);
    }

    async emergencyWithdraw(tokenLike: TokenLike, nonce: Nonce = "commited") {
        let token = await this.provider.resolveToken(tokenLike);
        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractInterface, this.ethWallet);
        let nonceNumber = await this.getNonce(nonce);
        let signature = this.walletKeys.signFullExit({token: token.id, eth_address: await this.ethWallet.getAddress(), nonce: nonceNumber});
        let tx = await franklinDeployedContract.fullExit(serializePointPacked(this.walletKeys.publicKey), token.address,  signature, nonceNumber,
            {gasLimit: utils.bigNumberify("500000"), value: utils.parseEther("0.02")});
        return tx.hash;
    }

    async transfer(to: Address, tokenLike: TokenLike, amount: utils.BigNumberish, fee: utils.BigNumberish, nonce: Nonce = "commited"): Promise<TransactionHandle> {
        let token = await this.provider.resolveToken(tokenLike);
        let tx = {
            from: this.address,
            to,
            token: token.id,
            amount,
            fee,
            nonce: await this.getNonce(nonce),
        };
        let signature = this.walletKeys.signTransfer(tx);
        let tx_req = FranklinProvider.prepareTransferRequestForNode(tx, signature);

        let submitResponse = await this.provider.submitTx(tx_req).catch(e => console.log(e));
        return new TransactionHandle(tx, submitResponse.hash, this.provider);
    }

    async close(): Promise<TransactionHandle> {
        let tx = {
            account: this.address,
            nonce: await this.getNonce()
        };

        let signature = this.walletKeys.signClose(tx);
        let tx_req = FranklinProvider.prepareCloseRequestForNode(tx, signature);

        let submitResponse = await this.provider.submitTx(tx_req);
        return new TransactionHandle(tx, submitResponse.hash, this.provider);
    }

    async getNonce(nonce: Nonce = "commited"): Promise<number> {
        if (nonce == "commited") {
            return (await this.provider.getState(this.address)).commited.nonce;
        } else if (typeof(nonce) == "number") {
            return nonce;
        } else if (nonce == "pending") {
            let state = await this.provider.getState(this.address);
            return state.commited.nonce + state.pending_txs.length;
        }
    }

    static async fromEthWallet(wallet: ethers.Signer, franklinProvider: FranklinProvider = new FranklinProvider()) {
        let seed = (await wallet.signMessage('Matter login')).substr(2);
        let ethAddress = await wallet.getAddress();
        let frankinWallet = new Wallet(Buffer.from(seed, 'hex'), franklinProvider, wallet, ethAddress);
        return frankinWallet;
    }

    // TODO: rename to getOnchainState ?
    async getOnchainBalances() {
        let tokens = await this.provider.getTokens();
        let onchainBalances = new Array<utils.BigNumber>(tokens.length);
        let contractBalances = new Array<utils.BigNumber>(tokens.length);

        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractInterface, this.ethWallet);
        for (let token of tokens) {
            if (token.id == 0) {
                onchainBalances[token.id] = await this.ethWallet.provider.getBalance(this.ethAddress);
            } else {
                const erc20DeployedToken = new Contract(token.address, IERC20ConractInterface, this.ethWallet);
                onchainBalances[token.id] = await erc20DeployedToken.balanceOf(this.ethAddress).then(n => n.toString());
            }
            contractBalances[token.id] = await franklinDeployedContract.balancesToWithdraw(this.ethAddress, token.id);
        }

        return {onchainBalances, contractBalances};
    }

    async getAccountState(): Promise<FranklinAccountState> {
        return await this.provider.getState(this.address);
    }
}
