import BN = require('bn.js');
import Axios from 'axios';
import {
    musigPedersen,
    privateKeyFromSeed,
    privateKeyToPublicKey,
    pubkeyToAddress, serializePointPacked, signTransactionBytes,
} from './crypto';
import {Contract, ContractTransaction, ethers, utils} from 'ethers';
import {packAmount, packFee} from "./utils";
import {curve} from "elliptic";

const IERC20Conract = require("../abi/IERC20.json");
const franklinContractCode = require("../abi/Franklin.json");
const priorityQueueInterface = new utils.Interface(require("../abi/PriorityQueue.json").interface);

export type Address = Buffer;
// Buffer or 0x prefixed hex string
export type AddressLike = Buffer | string;

// token, symbol/eth erc20 contract address, token id
export type TokenLike = Token | string | number;
export type Nonce= number | "commited" | "pending";

export function toAddress(addressLike: AddressLike): Address {
    if (typeof(addressLike) == "string") {
        return Buffer.from(addressLike.substr(2),"hex");
    } else {
        return addressLike;
    }
}

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

export class FranklinProvider {
    constructor(public providerAddress: string = 'http://127.0.0.1:3000', public contractAddress: string = process.env.CONTRACT_ADDR) {}

    static prepareTransferRequestForNode(tx: TransferTx, signature) {
        let req: any = tx;
        req.type = "Transfer";
        req.from = `0x${tx.from.toString("hex")}`;
        req.to = `0x${tx.to.toString("hex")}`;
        req.amount = utils.bigNumberify(tx.amount).toString();
        req.fee = utils.bigNumberify(tx.fee).toString();
        req.signature = signature;
        return req;
    }

    static prepareWithdrawRequestForNode(tx: WithdrawTx, signature) {
        let req: any = tx;
        req.type = "Withdraw";
        req.account = `0x${tx.account.toString("hex")}`;
        req.amount = utils.bigNumberify(tx.amount).toString();
        req.fee = utils.bigNumberify(tx.fee).toString();
        req.signature = signature;
        return req;
    }

    static prepareCloseRequestForNode(tx: CloseTx, signature) {
        let req: any = tx;
        req.type = "Close";
        req.account = `0x${tx.account.toString("hex")}`;
        req.signature = signature;
        return req;
    }

    async submitTx(tx) {
        return await Axios.post(this.providerAddress + '/api/v0.1/submit_tx', tx)
            .then(reps => reps.data)
    }

    async getTokens() {
        return await Axios.get(this.providerAddress + '/api/v0.1/tokens')
            .then(reps => reps.data)
    }

    async getTransactionsHistory(address: Address) {
        return await Axios.get(this.providerAddress + '/api/v0.1/account/' + `0x${address.toString("hex")}` + '/transactions')
            .then(reps => reps.data)
    }

    async getState(address: Address): Promise<FranklinAccountState> {
        return await Axios.get(this.providerAddress + '/api/v0.1/account/' + `0x${address.toString("hex")}`)
            .then(reps => reps.data)
    }

    async getTxReceipt(tx_hash) {
        return await Axios.get(this.providerAddress + '/api/v0.1/transactions/' + tx_hash)
            .then(reps => reps.data)
    }

    async getPriorityOpStatus(opId: number) {
        return await Axios.get(this.providerAddress + '/api/v0.1/priority_op/' + opId)
            .then(reps => reps.data)
    }

    async notifyPriorityOp(opId: number, action: "commit" | "verify") {
        return await Axios.get(this.providerAddress + `/api/v0.1/priority_op_notify/${action}/${opId}`)
            .then(reps => reps.data)
    }

    async notifyTransaction(hash: string, action: "commit" | "verify") {
        return await Axios.get(this.providerAddress + `/api/v0.1/tx_notify/${action}/${hash}`)
            .then(reps => reps.data)
    }

    async getBlockStatus(block: number) {
        return await Axios.get(this.providerAddress + '/api/v0.1/search?query=' + block)
            .then(reps => reps.data)
    }

    async resolveToken(token: TokenLike): Promise<Token> {
        if(typeof(token) == "string") {
            let tokens = await this.getTokens();
            let resolvedToken = tokens.find( (t, idx, arr) => {return t.symbol == token || t.address == token});
            if (resolvedToken) {
                return resolvedToken;
            } else {
                throw "Token address or symbol not found";
            }
        } else if (typeof(token) == "number") {
            let tokens = await this.getTokens();
            let resolvedToken = tokens.find( (t, idx, arr) => {return t.id == token});
            if (resolvedToken) {
                return resolvedToken;
            } else {
                throw "Token id not found";
            }
        } else {
            return token;
        }
    }
}

export interface Token {
    id: number,
    address: string,
    symbol?: string,
}

export interface FranklinAccountBalanceState {
    address: Address,
    nonce: number,
    balances: utils.BigNumber[],
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
    eth_address: String,
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
    eth_address: String
    nonce: number,
}

export class WalletKeys {
    publicKey: curve.edwards.EdwardsPoint;

    constructor(public privateKey: BN) {
        this.publicKey = privateKeyToPublicKey(privateKey);
    }

    signTransfer(tx: TransferTx) {
        let type = Buffer.from([5]); // tx type
        let from = tx.from;
        let to = tx.to;
        let token = Buffer.alloc(2);
        token.writeUInt16BE(tx.token,0);
        let bnAmount = new BN(utils.bigNumberify(tx.amount).toString() );
        let amount = packAmount(bnAmount);
        let bnFee = new BN(utils.bigNumberify(tx.fee).toString());
        let fee = packFee(bnFee);
        let nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(tx.nonce, 0);
        let msg = Buffer.concat([type, from, to, token, amount, fee, nonce]);
        return signTransactionBytes(this.privateKey, msg);
    }

    signWithdraw(tx: WithdrawTx) {
        let type = Buffer.from([3]);
        let account = tx.account;
        let eth_address = Buffer.from(tx.eth_address.slice(2),"hex")
        let token = Buffer.alloc(2);
        token.writeUInt16BE(tx.token,0);
        let bnAmount = new BN(utils.bigNumberify(tx.amount).toString());
        let amount = bnAmount.toArrayLike(Buffer, "be", 16);
        let bnFee = new BN(utils.bigNumberify(tx.fee).toString());
        let fee = packFee(bnFee);

        let nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(tx.nonce, 0);

        let msg = Buffer.concat([type, account, eth_address, token, amount, fee, nonce]);
        return signTransactionBytes(this.privateKey, msg);
    }

    signClose(tx: CloseTx) {
        let type = Buffer.from([4]);
        let account = tx.account;
        let nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(tx.nonce, 0);

        let msg = Buffer.concat([type, account, nonce]);
        return signTransactionBytes(this.privateKey, msg);
    }

    signFullExit(op: FullExitReq) {
        let type = Buffer.from([6]);
        let packed_pubkey = serializePointPacked(this.publicKey);
        let eth_address = Buffer.from(op.eth_address.slice(2),"hex")
        let token = Buffer.alloc(2);
        token.writeUInt16BE(op.token,0);
        let nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(op.nonce, 0);
        let msg = Buffer.concat([type, packed_pubkey, eth_address, token, nonce]);
        return Buffer.from(signTransactionBytes(this.privateKey, msg).sign, "hex");
    }
}

class DepositTransactionHandle {
    state: "Sent" | "Mined" | "Commited" | "Verified";
    priorityOpId?: utils.BigNumber;
    sideChainBlock?: number;

    constructor(public ethTx: ContractTransaction, public depositTx: DepositTx, public franklinProvider: FranklinProvider) {
        this.state = "Sent";
    }

    async waitTxMine() {
        if (this.state != "Sent") return;

        let txReceipt =  await this.ethTx.wait();
        for (let log of txReceipt.logs) {
            let priorityQueueLog = priorityQueueInterface.parseLog(txReceipt.logs[0]);
            if (priorityQueueLog) {
                this.priorityOpId = priorityQueueLog.values.serialId;
            }
        }
        if (!this.priorityOpId) {
            throw "Failed to parse tx logs";
        }

        this.state = "Mined"
    }

    async waitCommit() {
        await this.waitTxMine();
        if(this.state != "Mined") return;
        await this.franklinProvider.notifyPriorityOp(this.priorityOpId.toNumber(), "commit");
        this.state = "Commited";
    }

    async waitVerify() {
        await this.waitCommit();
        if(this.state != "Commited") return;

        await this.franklinProvider.notifyPriorityOp(this.priorityOpId.toNumber(), "verify");
        this.state = "Verified";
    }
}

class TransactionHandle {
    state: "Sent" | "Commited" | "Verified";
    txReceipt?;

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
        this.address = pubkeyToAddress(this.walletKeys.publicKey);
    }

    async deposit(tokenLike: TokenLike, amount: utils.BigNumberish, fee: utils.BigNumberish = utils.parseEther("0.001")): Promise<DepositTransactionHandle> {
        let token = await this.provider.resolveToken(tokenLike);
        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
        let contractTx;
        if (token.id == 0) {
            let totalAmount = utils.bigNumberify(amount).add(fee);
            contractTx = await franklinDeployedContract.depositETH(amount, this.address, {value: totalAmount, gasLimit: utils.bigNumberify("200000")});
        } else {
            const erc20DeployedToken = new Contract(token.address, IERC20Conract.interface, this.ethWallet);
            await erc20DeployedToken.approve(franklinDeployedContract.address, amount);
            const contractTx = await franklinDeployedContract.depositERC20(erc20DeployedToken.address, amount, this.address,
                {gasLimit: utils.bigNumberify("300000"), value: fee});
        }
        return new DepositTransactionHandle(contractTx, {to: this.address, amount, token}, this.provider);
    }

    async widthdrawOnchain(tokenLike: TokenLike, amount: utils.BigNumberish): Promise<ContractTransaction> {
        let token = await this.provider.resolveToken(tokenLike);
        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
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
        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
        let nonceNumber = await this.getNonce(nonce);
        let signature = this.walletKeys.signFullExit({token: token.id, eth_address: await this.ethWallet.getAddress(), nonce: nonceNumber});
        let tx = await franklinDeployedContract.fullExit(serializePointPacked(this.walletKeys.publicKey), token.address,  signature, nonceNumber,
            {gasLimit: utils.bigNumberify("500000"), value: utils.parseEther("0.02")});
        return tx.hash;
    }

    async transfer(to: AddressLike, tokenLike: TokenLike, amount: utils.BigNumberish, fee: utils.BigNumberish, nonce: Nonce = "commited"): Promise<TransactionHandle> {
        let token = await this.provider.resolveToken(tokenLike);
        let tx = {
            from: this.address,
            to: toAddress(to),
            // TODO: fix
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

    async getOnchainBalances() {
        let tokens = await this.provider.getTokens();
        let onchainBalances = new Array<utils.BigNumber>(tokens.length);
        let contractBalances = new Array<utils.BigNumber>(tokens.length);

        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
        for (let token of tokens) {
            if (token.id == 0) {
                onchainBalances[token.id] = await this.ethWallet.provider.getBalance(this.ethAddress);
            } else {
                const erc20DeployedToken = new Contract(token.address, IERC20Conract.interface, this.ethWallet);
                onchainBalances[token.id] = await erc20DeployedToken.balanceOf(this.ethAddress).then(n => n.toString());
            }
            contractBalances[token.id] = await franklinDeployedContract.balancesToWithdraw(this.ethAddress, token.id);
        }

        return {onchainBalances, contractBalances};
    }
}
